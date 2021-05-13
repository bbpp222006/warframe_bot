extern crate strsim;

use crossbeam::channel::{bounded, select, Receiver, Sender};
use num_format::{Locale, ToFormattedString};
use regex::Regex;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::fs;
use std::thread;
use std::time::Duration;
use strsim::jaro;

pub fn temp_item_json() -> String {
    let text = fs::read_to_string("item.json").unwrap();
    text
}

pub fn update_db() -> HashMap<String, String> {
    let client = reqwest::blocking::Client::new();
    let url = format!(
        "https://quiet-shape-5120.heroku11451.workers.dev/v1/items",
    );
    let res = client.get(url).header("Language", "zh-hans").send().unwrap().text().unwrap();
    // let res = temp_item_json();
    let v: Value = serde_json::from_str(&res).unwrap();

    let db_hash = json_2_hash(v.to_string());
    println!("更新完成");
    db_hash
}

pub fn json_2_hash(str_json: String) -> HashMap<String, String> {
    let v: Value = serde_json::from_str(&str_json).unwrap();
    let aval_vac: HashMap<String, String> = v["payload"]["items"]
        .as_array()
        .unwrap()
        .into_iter()
        .map(|value| {
            (
                value["item_name"].as_str().unwrap().to_string(),
                value["url_name"].as_str().unwrap().to_string(),
            )
        })
        .collect();

    aval_vac
}

fn get_score(dic_name: &str, search_name: &str) -> u64 {
    let mut pattern = dic_name.to_owned();
    let mut score = 0;

    fn circle_string(string: &mut String) {
        let a = string.pop().unwrap();
        string.insert(0, a);
    }

    for i in 0..dic_name.chars().count() - 1 {
        let temp_score = (jaro(&pattern, search_name) * 100.0) as u64;
        if temp_score >= score {
            score = temp_score
        }
        circle_string(&mut pattern);
    }
    score
}
//中    英                           //中    英    分数
pub fn get_rank(all_item_dic: &HashMap<String, String>, name: &str) -> Vec<(String, String, u64)> {
    let mut b: Vec<(String, String, u64)> = all_item_dic
        .into_iter()
        .map(|item| {
            (
                item.0.as_str().to_owned(),
                item.1.as_str().to_owned(),
                get_score(item.0, name),
            )
        })
        .collect();

    b.sort_by(|a, b| b.2.cmp(&a.2));
    b
}

pub fn get_single_price(item_name: &str) -> Vec<(String, String, String)> {
    println!("get请求查询：{}", item_name);
    let client = reqwest::blocking::Client::new();
    let url = format!(
        "https://quiet-shape-5120.heroku11451.workers.dev/v1/items/{}/orders",
        item_name
    );
    let res = client.get(url).send().unwrap().text().unwrap();
    let v: Value = serde_json::from_str(&res).unwrap();

    // println!("返回json：{}", v);
    let mut buy_hash: Vec<(String, String, String)> = v["payload"]["orders"]
        .as_array()
        .unwrap()
        .into_iter()
        .filter(|value| {
            let order_type = value["order_type"].as_str().unwrap();
            let online_status = value["user"]["status"].as_str().unwrap();
            order_type == "sell".to_string() && online_status != "offline"
        })
        .into_iter()
        .map(|value| {
            (
                value["user"]["ingame_name"].as_str().unwrap().to_string(),
                value["user"]["status"].as_str().unwrap().to_string(),
                value["platinum"].as_f64().unwrap().to_string(),
            )
        })
        .collect();

    buy_hash.sort_by(|a, b| a.2.cmp(&b.2));

    buy_hash
}

pub fn pretty_str(price_vec: Vec<(String, String, String)>, name: &str) -> Option<String> {
    let mut defalt_translate = HashMap::new();

    defalt_translate.insert("ingame", "游戏中");
    defalt_translate.insert("online", "在线  ");

    if price_vec.len() == 0 {
        return None;
    }

    let mut return_str = String::from(format!("{} 查询结果\n", name));

    for (num, (ingame_name, status, platinum)) in price_vec.into_iter().enumerate() {
        if num > 10 {
            break;
        }
        println!("{:?}", status);
        return_str.push_str(&format!(
            "{}  {}  {}\n",
            ingame_name, defalt_translate[&status as &str], platinum
        ));
    }
    Some(return_str)
}

pub fn filter_price(price_vec: Vec<(String, (f64, f64, u64))>) -> Vec<(String, (f64, f64, u64))> {
    let mut return_vec = vec![];
    let tuzhuang_reg = Regex::new(r"涂装").unwrap();
    let mut current_score = price_vec.get(0).map_or(0, |a| a.1 .2);
    // let mut max_num = 0;
    for (item_name, (sell, buy, score)) in price_vec.into_iter() {
        println!("{},{},{},{}", item_name, sell, buy, score);
        let score_gap = current_score - score;
        if score_gap > 9 {
            break;
        }
        if (tuzhuang_reg.is_match(&item_name) && score < 90) || (sell == 0.0 && score < 70) {
            continue;
        }

        current_score = score;
        return_vec.push((item_name, (sell, buy, score)));
    }
    return_vec
}
