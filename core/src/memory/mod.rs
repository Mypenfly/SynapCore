pub mod emdedding;
pub mod mem;

// #[cfg(test)]
// mod test {
//     use crate::{memory::mem::{MemoryConfig, MemoryStore}, read_config::{JsonConfig, LLMConfig}};

//     #[tokio::test]
//     async fn test() {
//         let config = MemoryConfig{
//             min_score:0.05,
//             boost:0.005,
//             penalty:0.005,
//             threshold:0.9,
//             top_k:5,
//             high_limit:5,
//         };

//         let path ="/home/mypenfly/projects/synapcore/.config/config.json" ;
//         let jsoncofig = JsonConfig::from_file(path).unwrap();
//         let llm_config =jsoncofig.get_config("siliconflow", "qwen_embed").unwrap() ;
//         // println!("llm_config:\n{:#?}",llm_config);

//         let store =match MemoryStore::open("/home/mypenfly/projects/synapcore/.config/memory/test.db", llm_config){
//             Ok(s) => s,
//             Err(e) => {
//                 eprintln!("store open falied:{}",e);
//                 return;
//             }
//         };

// //         if let Err(e) = store.store("我是Mypenfly,使用nixos,helix和wezterm").await{
// //             println!("store error: {:#?}",e);
// //             }
// //         if let Err(e) = store.store("我是devid,使用windows,vscode").await{
// //             println!("store error: {:#?}",e);
// //             }
// //         if let Err(e) = store.store("我是Mypenfly,喜欢动漫，最喜欢的是《无职转生》").await{
// //             println!("store error: {:#?}",e);
// //             }
// //         if let Err(e) = store.store("我是Mypenfly,我的这个项目叫synapcore").await{
// //             println!("store error: {:#?}",e);
// //             }
// //         if let Err(e) = store.store("我是Mypenfly,我有一只猫娘叫yore").await{
// //             println!("store error: {:#?}",e);
// //             }
// //         if let Err(e) = store.store("东京很美，我想去看看").await{
// //             println!("store error: {:#?}",e);
// //             }
// //         if let Err(e) = store.store("
// // API Key 认证失败。几个可能原因：").await{
// //             println!("store error: {:#?}",e);
// //             }

// //         let content = std::fs::read_to_string("/home/mypenfly/projects/synapcore/.config/data/Yore.json").unwrap();
// //         if let Err(e) = store.store(&content).await{
// //             println!("store error: {:#?}",e);
// //             }

// //         let content = std::fs::read_to_string("/home/mypenfly/notes/notes/FLY_System认知/helix运用.md").unwrap();
// //         if let Err(e) = store.store(&content).await{
// //             println!("store error: {:#?}",e);
// //             }

// //         let content = std::fs::read_to_string("/home/mypenfly/projects/synapcore/.config/prompts/Yore.md").unwrap();
// //         if let Err(e) = store.store("我是Mypenfly,使用nixos,helix和wezterm").await{
// //             println!("store error: {:#?}",e);
// //             }

// //         if let Err(e) = store.store("今天是2026年3月14日").await{
// //             println!("store error: {:#?}",e);
// //             }

//         let q = "我想知道我的技术水平";

//         let query = store.embedding_client.embed(q).await.unwrap();

//         let mem = store.search(&query, &config);

//         println!("result: {:#?}",mem);
//     }

// }
