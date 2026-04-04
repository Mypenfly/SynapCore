use std::path::Path;

use chrono::Utc;
use rusqlite::{params};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;
use r2d2_sqlite::SqliteConnectionManager;
use r2d2::Pool;

use crate::{
    memory::emdedding::{EmbeddingClient, EmbeddingErr},
    read_config::LLMConfig,
};

#[derive(Error, Debug)]
pub enum MemoryErr {
    #[error("Init erro:{0}")]
    Init(#[from] r2d2::Error),
    #[error("Databse erro:{0}")]
    Database(#[from] rusqlite::Error),
    #[error("Embedding falied:{0}")]
    Embedding(EmbeddingErr),
    #[error("Json convert falied:{0}")]
    Json(serde_json::Error),
}

pub type Result<T> = std::result::Result<T, MemoryErr>;

///记忆的数据结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Memory {
    pub id: String,
    pub content: String,
    pub score: f32,
    pub created_time: i64,
}

#[derive(Debug, Clone)]
pub struct MemoryStore {
    pool:Pool<SqliteConnectionManager> ,
    pub embedding_client: EmbeddingClient,
}

impl MemoryStore {
    pub fn open<P: AsRef<Path>>(path: P, config: LLMConfig) -> Result<Self> {
        //注册 squilte-vec 拓展
        unsafe {
            rusqlite::ffi::sqlite3_auto_extension(Some(std::mem::transmute(
                sqlite_vec::sqlite3_vec_init as *const (),
            )));
        }

        let manger = SqliteConnectionManager::file(path);
        let pool = Pool::new(manger).map_err(MemoryErr::Init)?;

        let store = Self {
            pool,
            embedding_client: EmbeddingClient::new(config),
        };

        store.init_tables()?;

        Ok(store)
    }

    fn init_tables(&self) -> Result<()> {
        let conn = self.pool.get().map_err(MemoryErr::Init)?;
        
        conn.execute(
            "CREATE TABLE IF NOT EXISTS memories(
                id    TEXT PRIMARY KEY,
                content    TEXT NOT NULL,
                score    REAL DEFAULT 0.5,
                created_time    INTEGER
            )",
            [],
        )?;

        conn.execute(
            &format!(
                "CREATE VIRTUAL TABLE IF NOT EXISTS vec_memories USING vec0(
                id TEXT PRIMARY KEY,
                embedding FLOAT[{}]
            )",
                1024
            ),
            [],
        )?;

        Ok(())
    }

    ///储存
    pub async fn store(&self, input: &str) -> Result<Memory> {
        let embedding = self
            .embedding_client
            .embed(input)
            .await
            .map_err(MemoryErr::Embedding)?;
        let embedding_json = serde_json::to_string(&embedding).map_err(MemoryErr::Json)?;

        let now = Utc::now().timestamp();
        let id = Uuid::new_v4().to_string();
        let score = 0.5;

        
        let conn = self.pool.get().map_err(MemoryErr::Init)?;
        //填表
        conn.execute(
            "INSERT INTO memories (
                id, content, score, created_time
            )
            VALUES(?1, ?2, ?3, ?4)",
            params![id, input, score, now],
        )?;

        // let embedding_bytes:&[u8] = unsafe {
        //     std::slice::from_raw_parts(embedding.as_ptr() as *const u8, embedding.len() * std::mem::size_of::<f32>())
        // };

        conn.execute(
            "
                INSERT INTO vec_memories (id, embedding) VALUES(?1, ?2)
            ",
            params![id, embedding_json],
        )?;

        Ok(Memory {
            id,
            content: input.to_string(),
            score,
            created_time: now,
        })
    }

    ///核心的检索算法
    pub fn search(&self, query: &[f32], config: &MemoryConfig) -> Result<Vec<SearchResult>> {
        let mut list = Vec::new();

        let embedding_json = serde_json::to_string(query).map_err(MemoryErr::Json)?;

        let sql = format!(
            "
                SELECT
                m.id, m.content, m.score, m.created_time,
                v.distance
                FROM vec_memories v
                JOIN memories m ON v.id = m.id
                WHERE v.embedding MATCH ?1
                AND k={}
                AND m.score >= ?2
                ORDER BY v.distance ASC
            ",
            config.top_k
        );

        // let mut stmt = self.conn.prepare("SELECT
        //         m.id, m.content, m.score,m.created_time,
        //         v.distance
        //         FROM vec_memories v
        //         JOIN memories m ON v.id = m.id
        //         WHERE v.embedding MATCH ?1
        //         AND k = ?2
        //         AND m.score >= ?3
        //         ORDER BY v.distance ASC
        //         ")?;
        let conn = self.pool.get().map_err(MemoryErr::Init)?;
        let mut stmt = conn.prepare(&sql)?;
        // println!("stage 1 - search passed!!");

        let results = stmt.query_map(params![embedding_json, config.min_score], |row| {
            let id: String = row.get(0)?;
            let content: String = row.get(1)?;
            let score: f32 = row.get(2)?;
            let created_time: i64 = row.get(3)?;
            let distance: f32 = row.get(4)?;

            let similarity = 1.0 - distance;
            let final_score = similarity * score;

            Ok(SearchResult {
                memory: Memory {
                    id,
                    content,
                    score,
                    created_time,
                },
                similarity,
                final_score,
            })
        })?;

        let mut hit_ids = Vec::new();
        for result in results {
            let result = result?;

            let id = result.memory.id.clone();

            hit_ids.push(id);

            list.push(result);
        }

        // println!("raw result:{:#?}", &list);

        self.boost(&hit_ids, config.boost)?;
        // println!("boost passed!!!");

        self.decay(&hit_ids, config.penalty)?;
        // println!("decay passed !!!");

        list.sort_by(|a, b| b.final_score.partial_cmp(&a.final_score).unwrap());

        list.truncate(config.top_k);

        //注入高分
        let high_score = self.get_high_score(config.threshold, config.high_limit)?;

        for mem in high_score {
            if !list.iter().any(|r| r.memory.id == mem.id) {
                list.push(SearchResult {
                    memory: mem,
                    similarity: 0.0,
                    final_score: 1.0,
                });
            }
        }

        //低分处理
        self.purge(config.min_score)?;

        Ok(list)
    }

    ///增长
    fn boost(&self, ids: &[String], boost: f32) -> Result<()> {
        if ids.is_empty() {
            return Ok(());
        }

        let placehoders: Vec<&str> = ids.iter().map(|_| "?").collect();

        let sql = format!(
            "UPDATE memories SET score = MIN(score * ( 1 + ?), 1.0) WHERE id IN ({})",
            placehoders.join(",")
        );

        let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(boost)];

        for id in ids {
            params_vec.push(Box::new(id.clone()));
        }

        let params_ref: Vec<&dyn rusqlite::ToSql> = params_vec.iter().map(|p| p.as_ref()).collect();

        let conn = self.pool.get().map_err(MemoryErr::Init)?;
        conn.execute(&sql, params_ref.as_slice())?;

        Ok(())
    }

    ///下降
    fn decay(&self, ids: &[String], penalty: f32) -> Result<()> {
        let sql = if ids.is_empty() {
            "UPDATE memories SET score = MAX(score * (1 - ?), 0.0)".to_string()
        } else {
            let placeholders: Vec<&str> = ids.iter().map(|_| "?").collect();
            format!(
                "UPDATE memories SET score = MAX(score * ( 1 - ?),0.0) WHERE id NOT IN ({})",
                placeholders.join(",")
            )
        };

        let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(penalty)];

        for id in ids {
            params_vec.push(Box::new(id.clone()));
        }

        let params_ref: Vec<&dyn rusqlite::ToSql> = params_vec.iter().map(|p| p.as_ref()).collect();

        let conn = self.pool.get().map_err(MemoryErr::Init)?;
        conn.execute(&sql, params_ref.as_slice())?;

        Ok(())
    }

    ///提取高分记忆
    fn get_high_score(&self, thresdhold: f32, limit: usize) -> Result<Vec<Memory>> {
        let conn = self.pool.get().map_err(MemoryErr::Init)?;
        let mut stmt = conn.prepare(
            "SELECT id, content, score, created_time
                FROM memories WHERE score >= ? ORDER BY score DESC LIMIT ?",
        )?;

        let results = stmt.query_map(params![thresdhold, limit as i32], |row| {
            Ok(Memory {
                id: row.get(0)?,
                content: row.get(1)?,
                score: row.get(2)?,
                created_time: row.get(3)?,
            })
        })?;

        let mut list = Vec::new();
        for result in results {
            list.push(result?);
        }
        Ok(list)
    }

    ///删除低分记忆
    fn purge(&self, thresdhold: f32) -> Result<()> {
        let conn = self.pool.get().map_err(MemoryErr::Init)?;
        let ids: Vec<String> = 
            conn
            .prepare("SELECT id FROM memories WHERE score < ?")?
            .query_map(params![thresdhold], |row| row.get(0))?
            .map(|r| r.unwrap_or("00000".to_string()))
            .collect();

        for id in ids {
            self.delete(&id)?;
        }
        Ok(())
    }

    fn delete(&self, id: &str) -> Result<()> {
        let conn = self.pool.get().map_err(MemoryErr::Init)?;
        conn
            .execute("DELETE FROM memories WHERE id = ?1", params![id])?;
        conn
            .execute("DELETE FROM vec_memories WHERE id = ?1", params![id])?;
        Ok(())
    }
}

#[derive(Debug)]
pub struct SearchResult {
    pub memory: Memory,
    pub similarity: f32,
    pub final_score: f32,
}

///记忆的参数配置
pub struct MemoryConfig {
    ///最低的分数限定
    pub min_score: f32,
    ///增长
    pub boost: f32,
    ///下降
    pub penalty: f32,
    ///高分注入量
    pub threshold: f32,
    ///高分取量
    pub high_limit: usize,
    ///检索结果量
    pub top_k: usize,
}
