# Rust 消消乐（Match-3）

用 Rust 编写、编译成 WebAssembly 在浏览器里运行的宝石消除小游戏（类 Bejeweled）。

- 8×8 棋盘，6 种宝石；开局保证没有现成的三连，并且至少有一步可走
- 点击选中再点相邻格交换，或直接拖动/滑动交换；交换后不能消除会自动换回
- 横/竖 3 个以上相同即消除 → 下落 → 顶部补充 → 连锁反应，连击倍数计分（每格 10 分 × 连击数）
- 无步可走时自动洗牌；每局 30 步，用完结束；最高分存 localStorage；有「提示」按钮

## 结构
```
Cargo.toml
src/game.rs     纯游戏逻辑（无浏览器依赖，含单元测试）
src/lib.rs      wasm-bindgen 导出
www/index.html  页面 + 少量 JS 胶水（渲染、输入、动画）
www/style.css
Dockerfile / nginx.conf.template / render.yaml
```

## 本地运行
```bash
rustup target add wasm32-unknown-unknown
cargo install wasm-pack        # 或用官方安装脚本
cargo test                     # 运行逻辑单元测试
wasm-pack build --release --target web --out-dir www/pkg --no-typescript
cd www && python3 -m http.server 8080
# 浏览器打开 http://localhost:8080
```
> 必须通过 HTTP 访问，不能直接双击打开 index.html。

## Docker 本地测试
```bash
docker build -t rust-match3 .
docker run --rm -p 10000:10000 rust-match3
# 打开 http://localhost:10000
```
镜像为两阶段构建：Rust 镜像里跑测试并用 wasm-pack 编译，再把静态文件交给 nginx；nginx 监听 `$PORT`（默认 10000），`.wasm` 以 `application/wasm` 返回。

## 部署到 Render
1. 把项目推到 GitHub 仓库。
2. Render 控制台 → **New → Blueprint**，选这个仓库，Render 会读取 `render.yaml`（Docker、免费方案）自动创建服务。
   或者 **New → Web Service**，选仓库，Runtime 选 **Docker**，Plan 选 Free。
3. 等构建完成后访问 Render 给的 `https://xxx.onrender.com`。
> 免费方案闲置会休眠，首次访问需要等几十秒唤醒；首次 Docker 构建约几分钟。
