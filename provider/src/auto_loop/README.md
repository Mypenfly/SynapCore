# AutoLoop

作为provider的核心模块，实现的功能是agent的自我进化功能

## AutoStudy

通过一次api调用，加入提示词例如：现在是AutoStudy模式，请你详细使用各式工具进行学习，内容包括但不限于最近和用户进行的交流，最近在做的项目，学习内容要使用skills_book工具记录，学习过程建议使用files_extract(学习现有项目)，web_search(查找有关资料)

### 实现逻辑
本质简单使用 core 下 task 工具来发起一次api请求，调用llm
我们考虑对 task发起时,is_save = false,即不进行保存，llm学习后自己会通过skills_book实现,

## AutoReflect

通过一次api调用(is_save = false),让agent自己反思，得出关于用户画像，
经验总结的内容，规定输出格式为<reflection>Content</reflection>，
我们通过提取<reflection>标签中的内容，并写入路径~/.config/synapcore/data/{character}_reflection.md中（.config路径通过dirs crate得到）

每次发起请求时我们注入现有的reflection文件，每次对话结束后我们都对reflection文件进行覆盖写入
> Content也有格式规定，写在privder/src/auto_loop/reflect_fmt.md中

## AutoClear

通过一次api调用(is_save = false),让agent对note_book,skills_book,的内容进行清理，建议是对已经失去效力或者长期不用的note,skill进行清理（建议清理数量为 >= 20）


## 实现细节考虑

1. 我们考虑要将Auto_loop环节的启动间隙(auto_loop_gap)通过Core.config.normal.auto_loop_gap从配置文件中获取，default=300 分钟
2. 我们考虑auto_loop集合到Provider中，具体做法是写一个auto_loop方法，并对Provider进行进一步扩展(具体看provider/README.md 中扩展建议)
3. 我们的auto_loop应该是一个轮询时间的循环，而为了解决每次用户启动应用停留时间不足gap,而导致我们的auto_loop设计无法生效，我们让AutoLoop持有一个计时器，
  实现一个exit安全退出方法，每次讲计时累计时间存入~/.cache/synapcore_cache/cache.json中（cache目录一样通过dirs crate获取），启动时读取

## 数据结构
```rust
struct AutoLoop{
  core:Core, //持有一个Core对象，不参与序列化
  time_count:usize,//计时器
  gap:usize,//间隔
}
```
