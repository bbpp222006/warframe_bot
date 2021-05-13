use serde_json::{json, Value};
use std::collections::HashMap;
use std::fs;
use strsim::jaro;
mod util;
use crossbeam::channel::{bounded, select, Receiver, Sender};
use regex::Regex;
use std::env;
use std::thread;
use std::time::Duration;


fn temp_order_json()->String{
    let text = fs::read_to_string("orders.json").unwrap();
    text
}

fn main() {
    thread::sleep(Duration::from_secs(1)); //延时5s启动
    // let ws_url =env::var("WS").unwrap();
    let ws_url = "ws://106.15.91.156:6700"; //"ws://10.243.184.136:30010";

    let (socket_send_tx, message_out) = util::create_socket_channel(&ws_url);
    let (update_sig_tx, update_sig_rx) = bounded(1);
    let (item_search_tx, item_search_rx): (
        Sender<std::string::String>,
        Receiver<std::string::String>,
    ) = bounded(10);
    let (search_result_tx, search_result_rx) = bounded(10);

    let update_sig_cron = update_sig_tx.clone();
    let cron_loop = thread::spawn(move || {
        //定时更新循环
        loop {
            thread::sleep(Duration::from_secs(60 * 60 * 24)); //一天更新一次
            println!("触发定时更新");
            
            update_sig_cron.send(1).unwrap();
        }
    });

    let item_search_db_rx = item_search_rx.clone();
    let database_loop = thread::spawn(move || {
        //数据库查询和更新模块
        let mut db_hash = HashMap::new();
        loop {
            select! {
                recv(update_sig_rx) -> _ =>{
                    println!("收到更新信号，将进行数据库更新");
                    db_hash =util::update_db();
                },
                recv(item_search_db_rx) -> item =>{
                    let name = item.unwrap();
                    println!("收到查询信号，查询目标{}",name);
                    let names = util::get_rank(&db_hash, &name);
                    let price = util::get_single_price(&names[0].1);
                    // let return_str = util::pretty_str(price);
                    search_result_tx.send((price,names[0].0.to_owned())).unwrap();
                }
            }
        }
    });

    let message_out_wf = message_out.clone();
    let socket_send_tx_wf = socket_send_tx.clone();
    let wf_re = Regex::new(r"^wf +([^ ]+)").unwrap();
    let wf_loop = thread::spawn(move || {
        // wf模块
        loop {
            let raw_message = message_out_wf.recv().unwrap();
            let v: Value = serde_json::from_str(&raw_message).unwrap();

            if let Some(v_) = wf_re.captures(v["message"].as_str().map_or("", |x| x)) {
                let user_id = v["user_id"].as_u64().unwrap();
                let group_id = v["group_id"].as_u64().unwrap();

                let item_name = v_.get(1).unwrap().as_str();
                println!("查询目标：{}", item_name);

                item_search_tx.send(item_name.to_owned()).unwrap();

                let (price_detail_vec,name) = search_result_rx.recv().unwrap();
                let str_to_send = util::pretty_str(price_detail_vec,&name).map_or(
                    r#"听不懂你在嗦森莫"#
                        .to_owned(),
                    |x| x,
                );
                
                let message_to_send = json!({
                    "action": "send_group_msg",
                    "params": {
                        "group_id": group_id,
                        "message": str_to_send,
                    },
                    "echo": "123"
                })
                .to_string();
                socket_send_tx_wf.send(message_to_send).unwrap();
            } else {
                println!("没有匹配到")
            }
        }
    });

    let update_sig_setup = update_sig_tx.clone();
    update_sig_setup.send(1).unwrap(); //第一次更新

    println!("启动成功");
    let _ = wf_loop.join();
    let _ = cron_loop.join();
    let _ = database_loop.join();
    println!("Exited");

    // let a = util::get_single_price("void_cloak");
    // println!("{:?}", a[0]);
}
