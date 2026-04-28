# UI 交互逻辑

主要使用鼠标点击，滚动 和 键盘操作逻辑

## run

启动：
1. 是一个线程进行provider的run,和对provider传出的监听
2. 监听用户的键盘，鼠标操作(handle_operate)
3. 启动页面渲染逻辑（draw_worker）

## 数据持有

1. state: 记录app状态
2. page: 记录当前渲染界面
3. theme: 记录主题配置和各个页面共享

## theme
主题配置在 ./theme 中

目前实现两个主题
一个是 everyforest 暗色风格主题(默认)
一个是 one_dark 暗色主题风格 

包含的主要是markdown的渲染
