1. 这是一个eslint规则，用于检查零宽字符
const noZerowidthChars = {
  meta: {
    type: 'problem',
    docs: {
      description: '存在零宽字符，请检查',
      category: 'Possible Errors',
    },
    fixable: 'code',
    schema: [],
  },
  create: function (context: any) {
    return {
      Identifier: function (node: any) {
        const regex = /[\u200B-\u200D\uFEFF]/;
        if (regex.test(node.name)) {
          context.report({
            node: node,
            message: '存在零宽字符，请检查',
          });
        }
      },
      Literal: function (node: any) {
        const regex = /[\u200B-\u200D\uFEFF]/;
        if (typeof node.value === 'string' && regex.test(node.value)) {
          context.report({
            node: node,
            message: '存在零宽字符，请检查',
          });
        }
      },
      Property: function (node: any) {
        const regex = /[\u200B-\u200D\uFEFF]/;
        if (regex.test(node.key.name)) {
          context.report({
            node: node,
            message: '存在零宽字符，请检查',
          });
        }
      },
    };
  },
};

export default noZerowidthChars;
2. 在crates/oxc_linter/src/rules/eslint目录下生成这个规则，编写风格参考，同目录下的其他规则
3. 包含完整的说明，impl Rule for的具体实现，#[test]
4. 注意在crates/oxc_linter/src/rules.rs里完成新规则对应代码
5. 注意不要写在crates/oxc_linter/src/generated/rule_runner_impls.rs里。这个文件是自动生成的。请不要执行`cargo run -p oxc_linter_codegen`