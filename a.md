# oxlint JS插件实现与Rust-JS交互原理深度解析

## 1. 引言

### 1.1 oxlint的定位与技术优势
- **定位**：oxlint 是 Oxc 工具链中的高性能 JavaScript/TypeScript Linter，复用 Rust 解析与语义分析能力，目标是在大型仓库中替换 ESLint 等传统方案。
- **技术特点**：Rust 端负责解析、作用域和规则执行的主流程，提供多线程并行、零拷贝 AST 访问等能力；JS 插件层提供与 ESLint 兼容的规则编写体验与生态扩展能力。
- **为什么需要 JS 插件**：存量 ESLint 规则丰富，通过 JS 插件系统可以直接复用生态并支持团队按熟悉方式扩展，避免完全迁移到 Rust 编写规则的成本。

### 1.2 文档阅读指南
- **适用读者**：具备 Rust/Node.js 开发经验、熟悉 ESLint 规则机制的工程师。
- **结构**：第 2 章介绍 JS 插件运行时代码，第 3–5 章聚焦 Rust-JS 交互与内存安全，第 6 章分析性能策略，第 7 章提供插件实战，第 8 章给出展望。
- **阅读建议**：若只关心插件接口可重点看 2.2 和 7 章；若关注跨语言机制，建议配合第 3–5 章的代码定位阅读。

## 2. oxlint JS插件系统架构

### 2.1 核心目录结构
JS 插件运行时全部位于 `apps/oxlint/src-js/plugins/`，目录结构如下：
```
comments.ts      # 注释收集与查询
context.ts       # 规则执行上下文（report、fix、sourceCode等）
fix.ts           # FIXER 实现，生成 range+text 补丁
index.ts         # loadPlugin/lintFile 入口聚合
lint.ts          # 整个 lint 流程、缓冲管理、AST 遍历
load.ts          # 插件装载、规则注册、create/createOnce 处理
location.ts      # 行列偏移换算
scope.ts         # ScopeManager/变量/引用模型
selector.ts      # 选择器 DSL（与 ESLint ASTSelector 兼容）
source_code.ts   # 源码+AST 懒加载、visitorKeys
tokens.ts        # Token 切片与计数工具
types.ts         # Node/Token/Visitor 等类型定义
utils.ts         # 错误格式化、断言
visitor.ts       # 多规则 visitor 编译与 Program walk
```
构建后这些模块被打包到 `npm/oxlint/dist/plugins.js` 中（见生成文件开头的导入语句和 visitor key 常量）。

### 2.2 核心模块功能解析

#### 2.2.1 插件加载系统（`load.ts`）
- **Plugin/Rule 接口**：`Plugin` 包含 `meta` 与 `rules` 字典；`Rule` 支持传统 `create(context)` 与 oxlint 扩展的 `createOnce(context)`（一次性构建 visitor 与 before/after hook）。
- **注册机制**：`registeredPluginPaths` 防止重复加载；每个 rule 会创建 `Context` 并写入 `registeredRules` 数组，其索引就是 Rust 端传入的 ruleId（`load.ts:122-187`）。
- **createOnce 处理**：对返回的 visitor 拆分 `before/after` 钩子，若 visitor 为空则注入 `neverRunBeforeHook` 以避免无意义调用。
- **ESLint 兼容性**：`apps/oxlint/src-js/index.ts` 暴露 `definePlugin/defineRule`，会为只有 `createOnce` 的规则注入兼容的 `create`，方便在 ESLint 中复用。

#### 2.2.2 规则执行上下文（`context.ts`）
- `Context` 保存 rule id、文件路径、options、是否可自动修复等内部数据，并在 `setupContextForFile` 中注入当次 lint 的信息（`context.ts:41-154`）。
- `context.report` 支持 `node`/`loc` 两种定位方式，能解析 `messageId` 与占位符，最终写入 `diagnostics` 数组，等待返回 Rust（`context.ts:151-205`）。
- `context.sourceCode` 暴露惰性加载的 `SOURCE_CODE` 对象，提供 `text/ast/lines/scopeManager` 等接口；`context.options` 将 CLI 配置与 ruleId 绑定。
- Fix 支持基于 `fix.ts` 的 `Fixer` API，最终转换为 `range + text` 形式交还 Rust。

#### 2.2.3 lint 主逻辑（`lint.ts`）
- Rust 传入 `(filePath, bufferId, buffer, ruleIds)`。JS 端使用 `buffers` 数组缓存传来的 `Uint8Array`，若 `buffer === null` 则重用旧实例（`lint.ts:19-105`）。
- `setupSourceForFile` 把缓冲区交给 `source_code.ts`，但只在真正需要 AST/源码时才解码，避免无谓成本（`lint.ts:89-100`）。
- 遍历 `ruleIds`，调用 `setupContextForFile`，并根据 rule 是否 createOnce 生成 visitor；`addVisitorToCompiled` / `finalizeCompiledVisitor` 把多个 visitor 合并，缺少目标节点时直接跳过 AST 遍历（`lint.ts:100-138`）。
- AST 遍历由生成的 `walkProgram` 驱动（`src-js/generated/walk.js`），完成后按顺序执行累积的 `afterHooks`，最后 `resetSourceAndAst()` 释放状态。

#### 2.2.4 AST 与源代码处理（`source_code.ts`）
- **缓冲结构**：`buffer` 同时被视作 `Uint8Array/Uint32Array/Float64Array`，通过常量 `DATA_POINTER_POS_32` 和 `SOURCE_LEN_OFFSET` 读取 AST 起点与源码长度（`source_code.ts:1-67`）。
- **反序列化**：`deserializeProgramOnly`（生成于 `apps/oxlint/dist/generated/deserialize.js`）根据缓冲中的 offset 构造带 `range/loc/parent` 的 ESTree 节点，以匹配 ESLint 期望结构。
- **SOURCE_CODE API**：暴露 `text/hasBOM/ast/scopeManager/visitorKeys/getText/getAncestors` 等常用方法，并在 `resetSourceAndAst` 中清理状态、防止下一次访问旧内存（`source_code.ts:89-198`）。

### 2.3 插件与规则接口定义
在 `load.ts` 中定义：
```ts
export interface Plugin {
  meta?: { name?: string };
  rules: { [key: string]: Rule };
}
export type Rule = CreateRule | CreateOnceRule;
export interface CreateRule {
  meta?: RuleMeta;
  create: (context: Context) => Visitor;
}
export interface CreateOnceRule {
  meta?: RuleMeta;
  create?: (context: Context) => Visitor;
  createOnce: (context: Context) => VisitorWithHooks;
}
```
其中 `RuleMeta` 支持 `fixable`、`messages` 等字段，与 ESLint 基本一致。

## 3. Rust与JS交互核心机制

### 3.1 N-API 架构选择与实现
- **N-API 简述**：Node.js 官方在 v8 引入的稳定原生扩展 ABI，允许编写 C/C++/Rust 动态库由 JS 直接加载（官方设计目标：隔离 V8 版本变动）。
- **选择原因**：oxlint npm 版本需要在 Node 环境中运行 Rust 核心，不希望二进制与 Node 版本强耦合；N-API 还能通过 `napi-rs` 自动生成 TypeScript 绑定。
- **单进程优势**：与 “Rust CLI 进程 + JS IPC” 相比，N-API 模式下 Node 进程直接加载 `.node` 动态库，所以 Rust 与 JS 共享同一 address space，可以零拷贝共享 AST 缓冲区，也省去了进程间通信。

### 3.2 关键回调接口设计
Rust 端在 `apps/oxlint/src/run.rs` 中把 JS 回调声明为 `ThreadsafeFunction`。结构如下：

#### 3.2.1 插件加载回调 `JsLoadPluginCb`
```rust
#[napi]
pub type JsLoadPluginCb = ThreadsafeFunction<
    FnArgs<(String, Option<String>)>, // 插件路径 + 可选包名
    Promise<String>,                  // JSON 形式的 PluginLoadResult
    FnArgs<(String, Option<String>)>,
    Status,
    false,
>;
```
- Rust 通过 `wrap_load_plugin` 调用该回调。因为 JS 侧实现是 async 函数，Rust 使用 `tokio::task::block_in_place` 和 `Handle::block_on` 等待 Promise 解析并把 JSON 结果转化为 `PluginLoadResult`（`apps/oxlint/src/js_plugins/external_linter.rs:32-54`）。

#### 3.2.2 文件 lint 回调 `JsLintFileCb`
```rust
#[napi]
pub type JsLintFileCb = ThreadsafeFunction<
    FnArgs<(String, u32, Option<Uint8Array>, Vec<u32>)>,
    String, // JSON 序列化后的 Vec<LintFileResult>
    FnArgs<(String, u32, Option<Uint8Array>, Vec<u32>)>,
    Status,
    false,
>;
```
- 对应的 `wrap_lint_file` 创建 `mpsc::channel` 等待 JS 返回，并将 `Uint8Array` 直接指向 Rust arena 的一块内存（`external_linter.rs:63-116`）。

### 3.3 ThreadsafeFunction 实现原理
- `ThreadsafeFunction` 是 `napi-rs` 对 N-API `napi_threadsafe_function` 的封装，允许在 Rust 任意线程向 JS 主线程排队任务。
- **跨线程保障**：Rust 线程在 `call_async` 或 `call_with_return_value` 时，只负责把参数序列化为 `FnArgs`；实际执行发生在 Node 的事件循环线程，避免并发访问 V8。
- **Promise/同步协作**：`call_async` 会返回 `Promise<String>`，Rust 等待其 resolved；同步 `lintFile` 则通过 channel 阻塞直到回调执行完成。
- **错误传播**：一旦 JS 抛异常，N-API 会把状态返回为 `Status::GenericFailure`，`wrap_lint_file` 将其转换为 panic 或字符串错误，上层 CLI 决定是否终止。

## 4. 内存共享技术细节

### 4.1 Raw Transfer 常量
- 由 `tasks/ast_tools/src/generators/raw_transfer.rs` 生成 `raw_transfer_constants`，包括 `BLOCK_SIZE/BLOCK_ALIGN/BUFFER_SIZE` 等信息（`apps/oxlint/src/generated/raw_transfer_constants.rs`）。
- `AllocatorPool::new_fixed_size` 保证每个解析任务都在同样大小的 arena 上运行，方便 Rust 把整块内存暴露给 JS。

### 4.2 缓冲区分配与传递
- `wrap_lint_file` 调用 `get_buffer`（`external_linter.rs:118-194`）读取 `Allocator::fixed_size_metadata_ptr()`：
  - 若该 buffer 第一次发送，则创建 `Uint8Array::with_external_data`，注册自定义释放回调 `free_fixed_size_allocator`。
  - 若之前已经传输，`metadata.is_double_owned` 会返回 `true`，Rust 只发送 bufferId，JS 端从缓存数组中取出已有 `Uint8Array`，避免重复映射。
- 缓冲起始地址通过对齐计算 `chunk_ptr = metadata_ptr.cast::<u8>() - offset`，保障与 JS 视图一致。

### 4.3 JS 端缓冲视图
- 第一收到的 buffer 会挂上 `Uint32Array/Float64Array` 视图，供 AST 反序列化和位置信息解析复用（`lint.ts:63-80`）。
- `BufferWithArrays` 结构还保留原始 `Uint8Array`，供 `TextDecoder` 解码源码；JS 端不会修改缓冲内容，避免破坏 Rust 拥有的引用语义。
- `resetBuffer()`（`dist/generated/deserialize.js` 提供）会在每轮 lint 结束后清空反序列化状态，但不会释放底层 ArrayBuffer，保持双端一致。

### 4.4 AST 反序列化流水线
- `deserializeProgramOnly(buffer, sourceText, sourceByteLen, getNodeLoc)` 读取 Rust 写入的结构体描述，输出 ESTree 风格对象。`getNodeLoc` 来自 `location.ts`，对 `range` -> `loc` 的转换做懒加载。
- Node 缓冲使用 little-endian，与 Rust 编译目标一致；`Uint32Array`/`Float64Array` 访问按照 4/8 字节对齐读取，不会破坏数据完整性。
- 反序列化代码在构建时由 `tasks/ast_tools` 生成，可适配 AST schema 变化。

## 5. 内存安全保障体系

### 5.1 缓冲区状态管理
- `FixedSizeAllocatorMetadata` 含 `is_double_owned` 原子布尔，Rust 通过 `swap(true, Ordering::SeqCst)` 标记是否已发送给 JS，SeqCst 确保跨线程访问次序（`external_linter.rs:149-154`）。
- Buffer 仅在第一次共享时创建 JS 视图，之后始终双端共用同一块内存，避免多次映射导致的生命周期难题。

### 5.2 避免 use-after-free
- `Uint8Array::with_external_data` 的释放回调会在 JS GC 判定 ArrayBuffer 无引用后调用 `free_fixed_size_allocator`，但由于 `buffers[bufferId]` 永远保留对该实例的引用，实际上直到进程结束才释放，杜绝 JS 端提前回收。
- Rust 侧只有在 `AllocatorPool` 回收对应 chunk 时才会真正释放，且在回收前不会再将同一 chunk 交给 JS。

### 5.3 内存释放机制
- Rust CLI 退出时，`AllocatorPool` Drop 触发释放；Node 版本则依赖 `buffers` 长期持有，减少频繁 alloc/free。（未来计划：自研 allocator 以更精细地在每轮 lint 后回收）。
- 若未来支持 JS 主动释放，可在 `plugins/lint.ts` 为某些 bufferId 置空，从而允许 `buffers` 放弃引用，回调得以执行。

### 5.4 并发访问控制
- `ThreadsafeFunctionCallMode::NonBlocking` 保证 Rust 线程不会阻塞在 N-API 自身；真正阻塞发生在 `mpsc::channel.recv()` 上，可控且仅限于等待回调结束。
- JsLoadPluginCb 仅在 CLI 启动阶段调用少量次，JS 端串行处理；JsLintFileCb 会在每个 worker 线程上调用，但 JS 回调始终在 Node 主线程执行，因此不会出现多线程同时写 buffer 的情况。

## 6. 性能分析与优化

### 6.1 直接内存访问优势
- 零拷贝：Rust 解析后的 AST 不需要再序列化为 JSON，通过共享 `Uint8Array` 直接提供给 JS；相比 ESLint JSON AST（动辄几十 MB），大幅降低 CPU 和 GC 压力。
- TypedArray：`Uint32Array/Float64Array` 可被 V8 优化为连续内存访问，配合 `visitProgram` 遍历速度接近原生。

### 6.2 避免不必要开销
- 大量逻辑（如 try/catch、错误包装）被拆分到独立函数（`lint.ts` 的 `lintFileImpl`），确保 V8 可优化热路径。
- 缓冲与 `diagnostics`、`afterHooks` 数组都复用；`Context` 实例每个 rule 只创建一次，避免 per-file 构造。
- 通过 `finalizeCompiledVisitor` 在 Rust 侧预先过滤“空 visitor”规则，减少 AST 遍历时的回调分发。

### 6.3 并发处理能力
- Rust CLI 由 `CliRunner` 控制线程数量，多个 worker 并行解析不同文件；每个 worker 线程都可以通过 `ThreadsafeFunction` 调度 JS 规则。
- 缓冲池固定大小，避免频繁分配；`RawTransferFileSystem`（`apps/oxlint/src/js_plugins/raw_fs.rs`）将源码读入 allocator 起始位置，减少 memcpy。
- JS 端虽然在主线程执行，但每次 lint 只需处理一个文件的 visitor，瓶颈更多在规则复杂度而非桥接通信。

## 7. 插件开发实践

### 7.1 插件创建流程
1. 使用 npm/pnpm 初始化包，安装 `@ytk-oxlint/oxlint` 作为依赖。
2. 在插件入口里导出 `definePlugin({ rules: { ... } })`，并可选提供 `meta.name` 与 `RuleMeta`。
3. 通过 `oxlintrc.json` 的 `plugins`/`rules` 配置启用自定义规则。
4. 运行 `npx oxlint --import-plugin ./path/to/plugin.js` 验证效果，或在 ESLint 中通过 `definePlugin` 生成兼容版本。

### 7.2 规则实现示例
```ts
import { definePlugin } from '@ytk-oxlint/oxlint';

export default definePlugin({
  rules: {
    'no-console-log': {
      meta: {
        fixable: 'code',
        messages: {
          unexpected: 'Unexpected console.log() call',
        },
      },
      create(context) {
        return {
          CallExpression(node) {
            if (
              node.callee.type === 'MemberExpression' &&
              node.callee.object.type === 'Identifier' &&
              node.callee.object.name === 'console' &&
              node.callee.property.type === 'Identifier' &&
              node.callee.property.name === 'log'
            ) {
              context.report({ node, messageId: 'unexpected' });
            }
          },
        };
      },
    },
  },
});
```
- 该示例展示了如何使用 `context.report` + `messageId`，也表明 JS 插件 API 与 ESLint 高度一致。

### 7.3 调试与测试
- **本地调试**：`apps/oxlint/scripts/generate-packages.js` 可重新打包 npm 产物；也可以在 repo 根目录运行 `pnpm nx run oxlint:dev`（具体命令以仓库 scripts 为准）。
- **规则测试**：可借助 Vitest 或 ESLint RuleTester，或直接用 `oxlint --import-plugin` 配合 fixtures。
- **性能分析**：Rust 端可开启 `OXC_LOG` 环境变量；JS 端可在 `plugins/lint.ts` 内部打 `console.time`，但务必在提交前移除。

## 8. 总结与未来展望

### 8.1 技术实现总结
- oxlint npm 版本通过 N-API 在 Node 进程中加载 Rust 动态库，实现单进程、零拷贝的 JS 插件系统。
- JS 运行时与 ESLint 兼容，支持 createOnce、before/after hook 等扩展，同时保留熟悉的 Context/SourceCode/Fixer API。
- 内存共享依赖固定大小 allocator 与 bufferId 缓存机制，通过原子标记和双端引用保证安全。

### 8.2 未来优化方向
- **内存管理**：计划用自研 allocator 替换 bumpalo，使得 RawTransferFileSystem 的特殊读入逻辑可以回收；也能在 JS 端更细粒度释放。
- **插件 API**：扩展 parserServices、scope selectors、类型信息注入，让规则能访问 TypeScript 语义。
- **性能监控**：提供 per-rule 统计，帮助识别慢速 JS 插件；必要时在 Rust 侧做“热规则优先”调度。
- **WASM 探索**：未来可能在浏览器或 Edge Runtime 运行 oxlint，需要评估 WASM + JS 插件的接口设计。

---

> 资料来源：本文代码片段与路径来自本仓库（`apps/oxlint/src-js/`、`apps/oxlint/src/js_plugins/` 等）；N-API 定义与单进程模型介绍依据 Node.js 官方公开文档与社区通识。
