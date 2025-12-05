# oxlint JS插件实现与 Rust-JS 交互原理：N-API 与共享内存视角

本篇聚焦 npm 版本 oxlint 的跨语言栈：Node.js 通过 N-API 加载 Rust 动态库，如何把解析后的 AST 与上下文零拷贝交给 JS 插件，再从 JS 回传诊断结果。总体流程先简述，再把笔墨集中在 N-API 回调与共享内存协议两大核心。

## 1. 快速概览：JS 插件运行时
- **代码位置**：JS 插件运行时源代码位于 `apps/oxlint/src-js/plugins/`，经打包后落在 npm 包 `dist/plugins.js`。Rust 侧的外部 Linter 桥接则在 `apps/oxlint/src/js_plugins/`。
- **基本流程**：
  1. `npm/oxlint/bin/oxlint` 以 Node CLI 启动 `dist/cli.js`。
  2. `cli.js` 加载 `bindings.js`，得到 Rust 导出的 `lint` 函数。
  3. Node 传入 CLI 参数，并把 `loadPlugin`、`lintFile` 两个 JS 回调（懒加载自 `plugins/index.ts`）交给 `lint`。
  4. Rust CLI 运行主流程；需要 JS 插件时，通过 N-API 回调进入 JS，完成插件装载与规则执行。

除 N-API 与内存协议外的模块（context、visitor、scope 等）与 ESLint 相似，这里不再展开。

## 2. N-API 桥接：Rust 如何驱动 JS 插件

### 2.1 `#[napi]` 入口与线程模型
- `apps/oxlint/src/run.rs` 中的 `#[napi] pub async fn lint(...) -> bool` 是 Node 侧唯一导出；其签名包含 CLI 参数和两个回调类型。
- Rust CLI 在内部解析命令、调度文件。每个 worker 线程处理文件时都会调用这些回调，因此需要线程安全的桥梁。

### 2.2 回调类型：`ThreadsafeFunction`
`JsLoadPluginCb` 与 `JsLintFileCb` 都是 `ThreadsafeFunction<FnArgs<...>, ...>`（`run.rs:15-48`）：
- **loadPlugin**：参数是插件绝对路径与可选包名，返回一个 JSON 字符串。Rust 通过 `wrap_load_plugin` 把它转成阻塞函数：调用 `call_async` → 等待 Promise 完成 → 用 `serde_json` 解码为 `PluginLoadResult`（`apps/oxlint/src/js_plugins/external_linter.rs:32-54`）。
- **lintFile**：参数是 `(file_path, buffer_id, option<Uint8Array>, rule_ids)`，返回 JSON 化的 `Vec<LintFileResult>`。Rust 调用 `call_with_return_value` 并用 `mpsc::channel` 同步等待结果（`external_linter.rs:63-116`）。

`ThreadsafeFunction` 的关键点：
- Rust 任何线程都可调用；N-API 会在内部把回调排队到 Node 的主线程执行，避免直接跨线程访问 V8。
- 通过 `ThreadsafeFunctionCallMode::NonBlocking`，Rust 端把任务放入队列后立即返回；真正的等待发生在 `channel.recv()`，由我们掌控。

### 2.3 错误与状态传播
- 若 JS 抛错，N-API 会把 `Status` 设为非 `Ok`；`wrap_lint_file` 断言 `Status::Ok`，否则 panic，确保 CLI 以可预期方式终止。
- JS 回调返回 `{ Success | Failure }` 两种 JSON 结构；Rust 端统一解析，再根据 `Success(Vec<LintFileResult>)` 或 `Failure(String)` 继续执行或报错。

## 3. 共享内存协议：AST 如何零拷贝到 JS

### 3.1 固定大小 Arena 与缓冲标识
- oxlint 的解析器使用 `AllocatorPool::new_fixed_size` 生成固定大小的 `Allocator`。其元数据（`FixedSizeAllocatorMetadata`）记录 `id`、`is_double_owned` 等字段，常量值来自 `apps/oxlint/src/generated/raw_transfer_constants.rs`。
- `wrap_lint_file` 中的 `get_buffer`：
  1. 通过 `allocator.fixed_size_metadata_ptr()` 获取 metadata。
  2. 使用 `is_double_owned.swap(true, Ordering::SeqCst)` 判断该 buffer 是否已发送给 JS。
  3. 首次发送时，计算 chunk 起始地址（按 `BLOCK_ALIGN` 对齐），用 `Uint8Array::with_external_data(ptr, BUFFER_SIZE, drop_cb)` 创建 JS 视图。
  4. 返回 `(buffer_id, Option<Uint8Array>)` 给 JS。后续只传 `buffer_id`，由 JS 侧缓存复用。

### 3.2 JS 端的缓冲缓存与 AST 懒加载
- `apps/oxlint/src-js/plugins/lint.ts` 维护 `buffers: (BufferWithArrays | null)[]`。首次收到某个 `buffer_id` 时：
  - 保存 `buffer`，并派生 `Uint32Array`、`Float64Array` 视图；
  - 若再收到同 id 但 `buffer === null`，则直接从数组中取回原视图。
- `source_code.ts` 的 `setupSourceForFile` 接手该 `buffer`，但只在实际需要时才解码：
  - `initSourceText()` 使用 `TextDecoder` + `SOURCE_LEN_OFFSET` 读取源码；
  - `initAst()` 调用构建期生成的 `deserializeProgramOnly(buffer, sourceText, sourceByteLen, getNodeLoc)`；
  - `SOURCE_CODE` 对象 lazily 提供 `text/ast/visitorKeys/scopeManager` 等接口。

### 3.3 Rust ↔️ JS 的数据契约
1. **Rust → JS**：
   - 调用 `lintFile(file_path, buffer_id, buffer_opt, rule_ids)`；
   - `rule_ids` 是 Rust 侧根据配置挑选的规则索引，JS 用于查找 `registeredRules`。
2. **JS 内部执行**：
   - `setupContextForFile` 更新每个 rule 的 `Context`（包括 `options` 与 `isFixable`）。
   - 构建 visitor 队列并调用 `walkProgram(ast, compiledVisitor)`。
   - 将 `diagnostics` 格式化成 `{ message, start, end, ruleIndex, fixes }` 数组。
3. **JS → Rust**：
   - `lintFile` 返回 `JSON.stringify({ Success: diagnostics })`（若异常则 `Failure(error)`）。
   - Rust 解析后转换为 `Vec<LintFileResult>`，交给主 CLI 汇总。

### 3.4 为什么这样设计
- **零拷贝**：避免把 AST 转成 JSON（传统 ESLint 同步 AST 可能需要几十 MB）；`Uint8Array` 直接引用 Rust 内存，仅做视图转换。
- **安全性**：
  - 双端引用同一块内存，JS 永久持有 `buffers[bufferId]`，因此 `Uint8Array::with_external_data` 的 drop 回调不会被调用，消除 use-after-free。
  - Rust 只有在解析完文件后才重置 allocator，保证在 JS 遍历期间 no mutation。
- **性能**：
  - `rule` 的 `Context` 按规则复用，避免多次构造。
  - `finalizeCompiledVisitor` 可以检测没有访问任何节点的情况，直接跳过 AST 遍历，降低 JS 调度成本。

## 4. 小结与延伸
- npm 版本 oxlint 运行在 Node 进程，借助 N-API 把 Rust CLI 作为动态库嵌入；Rust 通过 `ThreadsafeFunction` 异步调度 JS 回调，既兼容异步插件加载，又保证主线程安全。
- 内存共享使用固定大小 allocator + bufferId 协议，让 JS 以 TypedArray 方式直接访问 Rust 构造的 AST/源码，彻底消除 JSON 序列化成本。
- 若未来需要扩展：
  - 可以在 JS 侧增加 buffer 生命周期管理，让长时间运行的守护进程释放不再使用的 buffer；
  - 在 Rust 端增加统计信息，评估 JS 插件耗时，进一步优化线程调度。

这套设计使 oxlint 能在保持 ESLint 插件体验的同时，充分发挥 Rust 解析与多线程优势，为大型代码库提供高吞吐的 lint 能力。
