# SkilslBook

工具skills_book记录agent skills

提供三个操作：
1. add,加入skill
2. remove,删除skill
3. read,读取skill

## 数据管理

skills目录在~/.config/synapcore/skills/{title}.md，配置目录通过dirs crate获得，不区分charater

## add
必须content,title两个参数
对content格式符合一般skill格式
写入skills目录
返回的格式是 Added skill {title}

agent 写入的content的内容中应该要有详细的操作流程，（其中也要有一个description来简要介绍使用，单独一行）
title即使一个skill标题也是一个简短的介绍

## remove
必须一个title,
根据title进行skill的删除
返回的格式是 Removed skill {title}

## read
传入title,
根据title,进行skill读取
返回内容

## 对外暴露一个api get_skills

检索目录下的所有skills,获取title

返回所有title
> 这一部分参考note_book中get_last中有关逻辑

