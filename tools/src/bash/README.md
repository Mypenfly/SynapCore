# 这是一个给agent的shell

## 设计要点

- Command的线程的持有是Bash,同时Bash由Tools持有，而在Bash被初次（一次core init内）调用时，
  该Command的线程对象被初次创建，并被持有，而通过线程的通信机制BashMessage来传入传出命令和输出
  
