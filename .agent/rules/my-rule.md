---
trigger: always_on
---

- 使用中文进行回复，思考过程使用英文
- 后端代码更改后，前端要同步进行相应的更改，可以自行判断
- 每次代码变更由使用者确认是否更新相关文档: README.md, USER_GUIDE.md, DEVELOPMENT.md, ARCHITECTURE.md, CHANGELOG.md
- 每次修改代码不要生成其他文档
- 每次修改代码不要写详细的变更说明，简洁的变更说明即可
- 不要随意删除代码，而是要深度思考过后的修改
- 确保项目代码“高内聚低耦合”特性，降低维护和开发成本
- 每次修改代码都要更新索引系统（索引文件+文件头注释），请遵循.agent/rules/doc-maintenance.md中的规则
- 代码搜索使用ace-tool，不要使用IDE搜索