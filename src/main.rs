use actix_web::{get, web, App, HttpResponse, HttpServer, Responder};
use chrono::prelude::*;
use rand::Rng;
use rusqlite::Connection;
use serde::Serialize;
use std::time::Instant;

#[derive(Serialize, Clone)]
pub struct ClockEntry {
    time: u32,
    text: String,
    author: String,
    title: String,
    link: String,
}

#[derive(Copy, Clone)]
pub struct Range {
    min: i32,
    max: i32,
}

impl Range {
    fn default() -> Range {
        Range { min: -1, max: -1 }
    }
}

fn clean_string(data: &String) -> String {
    return data
        .replace("\n", " ")
        .replace("\r", " ")
        .replace("\"", "")
        .replace("\\", "")
        .replace("_", "");
}
pub struct AppState {
    all_entries: Vec<ClockEntry>,
    time_index: Vec<Range>,
}

impl AppState {
    pub fn new(db_filename: &str) -> AppState {
        let fts_connection = Box::new(Connection::open(db_filename).unwrap());
        let mut stm = fts_connection
            .prepare("SELECT time, text, author, title, link FROM littime order by time;")
            .unwrap();
        let data = stm
            .query_map((), |row| {
                Ok(ClockEntry {
                    time: row.get(0).unwrap(),
                    text: row.get(1).unwrap(),
                    author: row.get(2).unwrap(),
                    title: row.get(3).unwrap(),
                    link: row.get(4).unwrap(),
                })
            })
            .unwrap()
            .map(|entry| entry.unwrap())
            .collect::<Vec<ClockEntry>>();

        let mut times: Vec<Range> = Vec::with_capacity(13 * 60);
        times.resize(13 * 60, Range::default());
        
        let mut prev_packed_index = 0 as usize;
        for (idx, time) in data.iter().enumerate() {
            let hour = time.time / 100;
            let minute = time.time % 100;
            let packed_index = (hour * 60 + minute) as usize;
            if packed_index != prev_packed_index || idx == 0 {
                let dif_index = packed_index - prev_packed_index;
                if dif_index > 1 {
                    for i in 1..dif_index {
                        times[prev_packed_index + i] = times[prev_packed_index];
                    }
                }
                times[packed_index as usize] = Range {
                    min: idx as i32,
                    max: idx as i32,
                };
            } else {
                times[packed_index as usize].max = idx as i32;
            }
            prev_packed_index = packed_index;
        }

        AppState {
            all_entries: data,
            time_index: times,
        }
    }

    fn get_entry(&self, hour: u32, minute: u32) -> ClockEntry {
        let h = match hour {
            0 => 12,
            1..=12 => hour,
            13.. => hour % 12,
        };

        let m = match minute {
            0..=59 => minute,
            60.. => minute % 60,
        };

        let now = Instant::now();
        let index = h * 60 + m;
        let range = self.time_index[index as usize];
        println!("range {} - {}", range.min, range.max);

        if range.min == range.max {
            return self.all_entries[range.min as usize].clone();
        }

        let index_show = rand::thread_rng().gen_range(range.min..range.max) as usize;

        println!("entry elapsed: {:.2?}", now.elapsed());
        return self.all_entries[index_show].clone();
    }

    fn get_html(&self, hour: u32, minute: u32) -> String {
        let now = Instant::now();
        let html_raw = include_str!("clock.html");

        let entry = self.get_entry(hour, minute);
        let mut html = html_raw.replace("{{author}}", clean_string(&entry.author).as_str());
        html = html.replace("{{title}}", clean_string(&entry.title).as_str());
        html = html.replace("{{link}}", &entry.link);
        html = html.replace("{{paragraph}}", &clean_string(&entry.text).as_str());
        html = html.replace("{{time}}", format!("{}:{}", hour, minute).as_str());
        println!("html elapsed: {:.2?}", now.elapsed());
        return html;
    }
}

#[get("/")]
async fn html_clock(data: web::Data<AppState>) -> impl Responder {
    let time = Local::now();
    let show = data.get_html(time.hour12().1, time.minute());
    HttpResponse::Ok().body(show)
}

#[get("{hour}/{minute}")]
async fn custom_html_clock(
    data: web::Data<AppState>,
    info: web::Path<(u32, u32)>,
) -> impl Responder {
    let show = data.get_html(info.0, info.1);
    HttpResponse::Ok().body(show)
}

#[get("/json")]
async fn json_clock(data: web::Data<AppState>) -> impl Responder {
    let time_now = Local::now();
    web::Json(data.get_entry(time_now.hour12().1, time_now.minute()))
}

#[get("/json/{hour}/{minute}")]
async fn custom_json_clock(
    data: web::Data<AppState>,
    info: web::Path<(u32, u32)>,
) -> impl Responder {
    web::Json(data.get_entry(info.0, info.1))
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    HttpServer::new(|| {
        App::new()
            .app_data(web::Data::new(AppState::new("lit_clock.db")))
            .service(html_clock)
            .service(json_clock)
            .service(custom_html_clock)
            .service(custom_json_clock)
    })
    .bind(("0.0.0.0", 8080))?
    .run()
    .await
}
