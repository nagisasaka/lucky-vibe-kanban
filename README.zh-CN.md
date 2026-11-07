# Lucky Vibe Kanban（LVK）

**通过看板规划任务，运行编码代理，并审查代码变更。**

LVK是自托管开发环境，提供Git工作树、多会话、可视化代理工作流、本地整合和OpenWiki仓库记忆。
可以在本机或自己的服务器运行，让代理在笔记本断开连接后继续工作。
代理账户、提供商订阅及模型费用需要单独配置。

[English](README.md) · [日本語](README.ja.md) ·
[源代码](https://github.com/nagisasaka/lucky-vibe-kanban) · [Apache-2.0](LICENSE)

## 发布方式

LVK仅通过GHCR发布Docker镜像，当前平台为Linux amd64，此分支不发布npm包。
旧npx启动器、Tauri桌面外壳及上游Cloud和relay服务已移除，请用浏览器访问LVK服务器URL。
外部Runner执行仍可通过Execution Bridge使用。

公开部署使用HTTPS及Basic／Auth0认证，首次启动保持锁定。
这是面向可信团队的环境，不是无限制的公共代理服务。
提供商和应用认证与LVK登录相互独立。数据持久化不保证中断进程自动恢复执行。

## 使用指南

- [服务器部署、证书、备份和恢复](openwiki/operations/server-container.md)
- [初始认证、成员和恢复](openwiki/concepts/server-access.md)
- [提供商登录、profile和MCP设置](openwiki/integrations/agent-providers.md)
- [从Issue到整合](openwiki/workflows/task-to-integration.md)与[自动合并](openwiki/concepts/formal-integration.md)
- [仓库记忆与正规Wiki维护](openwiki/operations/openwiki-maintenance.md)

## 源码开发

工具链、安装、构建、验证和恢复见[开发指南](openwiki/operations/development.md)，
贡献者规则见[AGENTS.md](AGENTS.md)。部署配置位于[deploy/server](deploy/server)，
镜像验证与发布由[publish-server.yml](.github/workflows/publish-server.yml)负责。发布不等于部署。

首次使用源码时，先安装仓库指定的工具链和`cargo-watch`，
然后依次执行`pnpm install --frozen-lockfile`和`pnpm run dev`。

README由人类在AI协助下维护。操作细节由AI维护的OpenWiki负责，并与已整合源码核对。
源码、测试和配置优先。LVK使用Wiki维护知识，不再建设独立文档网站。

## 名称与来源

产品名称为**Lucky Vibe Kanban**，简称**LVK**。当前配置和共享目录使用`LVK_*`和`.lvk/shared`。
原版Vibe Kanban的数据路径、`VK_*`配置和原生二进制名称保持不变。

LVK专用的仓库管理文件放在 `.lvk/` 中。`integration-validation.json`、`.gitignore`
以及LVK自身开发用的 `preview/` 可通过Git共享。只有本地的 `shared/` 链接和
`attachments/` 副本由 `.lvk/.gitignore` 排除；该文件不排除自身，保留用户规则并按需追加运行时规则。
`shared/tmp/` 用于同一注册仓库的Workspace间临时交接，`cache/` 用于可再生成的复用数据，
`persistent/` 用于持续保存的共享数据。Run结束不会删除tmp；仅在交接完成、不再需要且无人使用后整理。
人类批量整理也遵守相同条件。不设TTL或自动清理；必要成果应保存在适当的长期位置。
仓库外的共享存储和中央附件缓存保持原位。附件以Session的agent工作目录为基准，
未指定时使用Workspace根目录；共享链接按仓库隔离。
`AGENTS.md`（包含必需的LVK管理块）、`CLAUDE.md`、`openwiki/`、`.openwikiignore`
及外部工具配置保留标准位置。Wiki专用维护worktree保留已跟踪的ignore配置，
仅通过Git本地的 `info/exclude` 排除 `.lvk/shared/` 和 `.lvk/attachments/`。
该Git信息由worktree共享，但不排除源码或其他 `.lvk/` 配置；普通Workspace仍使用 `.lvk/.gitignore`。
旧路径与历史不自动迁移、改写或兼容查找。

感谢[Easy Vibe Kanban](https://github.com/toby1123yjh/easy-vibe-kanban)
和[Vibe Kanban](https://github.com/BloopAI/vibe-kanban)的贡献者，我们保留其贡献与历史。
项目使用[Apache-2.0](LICENSE)许可证，归属说明见[NOTICE](NOTICE)。
