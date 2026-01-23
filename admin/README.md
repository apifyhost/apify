# Apify Admin Dashboard

基于 React + TypeScript + Ant Design 的管理后台，参考 NocoBase 架构设计。

## 技术栈

- **核心框架**: React 18 + TypeScript
- **UI 组件**: Ant Design 5.x
- **状态管理**: Redux Toolkit + RTK Query
- **路由**: React Router DOM 6.x
- **样式**: Styled Components
- **构建工具**: Vite

## 快速开始

### 开发模式

```bash
cd admin
npm install
npm run dev
```

访问 http://localhost:5173 (开发服务器)

### 生产构建

```bash
cd admin
npm run build
```

构建产物输出到 `../target/admin/`，由 Control Plane 提供服务。

### 生产环境访问

启动 Control Plane 后访问：
- **Admin Dashboard**: http://localhost:4000/admin/
- **API 端点**: http://localhost:4000/apify/admin/*

所有请求统一通过 4000 端口，无需 CORS 配置。

### 生产构建

```bash
npm run build
```

## 功能模块

### 已实现

- ✅ 基础布局（侧边栏、顶部导航）
- ✅ 仪表板（统计数据展示）
- ✅ API 配置管理（CRUD）
- ✅ 数据源管理（CRUD）
- ✅ 监听器管理（CRUD）
- ✅ RTK Query 集成
- ✅ 响应式设计

### 待开发

- ⏳ 表结构管理
- ⏳ 用户认证与授权
- ⏳ API 测试工具
- ⏳ 实时日志查看
- ⏳ 性能监控
- ⏳ 主题切换

## 项目结构

```
admin/
├── src/
│   ├── core/                 # 核心模块
│   │   ├── Application.tsx   # 应用主入口
│   │   └── store/           # Redux store
│   ├── components/          # 通用组件
│   │   └── Layout/          # 布局组件
│   ├── pages/              # 页面组件
│   │   ├── Dashboard/      # 仪表板
│   │   ├── Apis/          # API 管理
│   │   ├── DataSources/   # 数据源管理
│   │   ├── Listeners/     # 监听器管理
│   │   └── Schemas/       # 表结构管理
│   ├── services/          # API 服务
│   │   └── api.ts        # RTK Query API
│   ├── App.tsx
│   └── main.tsx
├── package.json
├── tsconfig.json
└── vite.config.ts
```

## API 集成

### 开发模式
通过 Vite proxy 代理到 Control Plane：

```typescript
// vite.config.ts (开发模式)
server: {
  port: 5173,
  proxy: {
    '/apify': {
      target: 'http://localhost:4000',
      changeOrigin: true,
    },
  },
}
```

开发模式创建 `.env.local` 文件（可选）：

```bash
# API Key (默认已在代码中配置)
VITE_API_KEY=UZY65Nakvsd3
```

生产模式无需环境变量，API Key 在后端配置。eUrl: '/apify/admin'
```

Control Plane 路由：
- `/admin/*` → Admin Dashboard 静态文件
- `/apify/admin/*` → Control Plane API
- `/` → 重定向到 `/admin/`

## 环境变量

创建 `.env.local` 文件：

```bash
VITE_API_BASE_URL=http://localhost:4000
VITE_API_KEY=UZY65Nakvsd3
```

## 开发指南

### 添加新页面

1. 在 `src/pages/` 创建页面组件
2. 在 `src/core/Application.tsx` 添加路由
3. 在布局侧边栏添加菜单项

### 添加 API 接口

在 `src/services/api.ts` 中使用 RTK Query：

```typescript
endpoints: (builder) => ({
  getItems: builder.query<Item[], void>({
    query: () => '/items',
    providesTags: ['Items'],
  }),
})
```部署架构

```
┌─────────────────────────────────────────┐
│     Control Plane (Port 4000)          │
├─────────────────────────────────────────┤
│  Routes:                                │
│  • /admin/*     → Admin Dashboard UI   │
│  • /apify/admin/* → Management API     │
│  • /             → Redirect to /admin/ │
└─────────────────────────────────────────┘
```

**优势**：
- ✅ 单一端口，部署简单
- ✅ 无需 CORS 配置
- ✅ 前后端统一管理
- ✅ 生产环境友好

**开发流程**：
1. 开发：使用 `npm run dev` (5173 端口，热重载)
2. 构建：使用 `npm run build` (输出到 `target/admin/`)
3. 部署：启动 Control Plane 即可访问 DashboardLicense

MIT
