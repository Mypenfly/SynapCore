这是外部工具定义的规划书

# 预期配置文件的格式
一样写入tools.toml？

```toml
[[outer]]
name = "get_weather"
description = "获取天气"
enable = true
exec = ["python","./test.py"]
required = ["location"]

[outer.parameters."location"]
type = "string"
description = "查询的地点"

[outer.parameters."time"]
type = "string"
description = "查询的时间，非必须"
```

> 对于运行的外部程序而言应该直接接受参数不需要长短选项
