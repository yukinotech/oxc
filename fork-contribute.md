## rust部分

1. `crates/oxc_linter/src/rules/eslint/no_zero_width_chars.rs`里写新规则
2. `crates/oxc_linter/src/rules.rs`这个路径里注册新写的规则
3. 根目录执行`cargo run -p oxc_linter_codegen`，生成注册代码
4. `cargo test -p oxc_linter no_zero_width_chars`跑测试，看看测试样例是否成功
5. 测试样例成功，但执行失败，会生成一个快照，把snapshots文件夹里的快照后缀名的.new去掉
6. 更新`npm/oxlint/package.json`版本号
到此，代码部分完成

验证：
7. 根目录执行`cargo build --release -p oxc_language_server --target aarch64-apple-darwin`查看是否编译成功
8. 根目录执行`cargo build --release -p oxlint --features napi --target aarch64-apple-darwin`查看是否编译成功
9. 根目录执行`cargo install cross --git https://github.com/cross-rs/cross`,安装cross，用来交叉编译linux产物，用于cloud ide
10. 根目录执行`cross build --release -p oxc_language_server --target x86_64-unknown-linux-gnu`
11. 根目录执行`cross build --release -p oxlint --features napi --target x86_64-unknown-linux-gnu`


## js部分

1. 根目录执行`pnpm --filter oxlint run build`，即可在npm/oxlint/dist生成发包文件
2. 根目录执行`node npm/oxlint/scripts/generate-packages.js`，复制各个平台的发布文件
