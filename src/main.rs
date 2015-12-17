// rust-trends
//
// Flow:
//  * Collect trends from GitHub.
//  * Eliminate trends which have been featured within the last 14 days.
//  * Construct and issue a tweet for each remaining trend.
//  * For each successful tweet, record the trend to avoid duplication for the
//    next 14 days.
//
// TODO:
//  * Configuration from env variables.
//  * File store (rusqlite)
//  * Console opts (clap)
// #![feature(plugin)]
// #![plugin(clippy)]
extern crate hyper;
extern crate chrono;
extern crate serde;
extern crate serde_json;
extern crate rusqlite;

use std::io::{Read, Error, ErrorKind};
use std::fs::File;

use hyper::client::Client;
use hyper::header::{Headers, UserAgent};
use chrono::*;
use serde_json::Value;
use rusqlite::{SqliteConnection, SqliteError, SqliteRows};

const GITHUB_API_URL: &'static str = "https://api.github.com";
const STORE_PATH: &'static str = "trends.db";

pub struct Store {
    db: SqliteConnection,
    schema: Option<StoreSchema>,
}

pub struct StoreSchema {
    build: i32, /* created: Option<DateTime<UTC>>,
                 * last_run: Option<DateTime<UTC>> */
}

impl Store {
    fn new() -> Store {
        let result = rusqlite::SqliteConnection::open(STORE_PATH);
        Store {
            schema: None,
            db: result.expect("Failed to open store"),
        }
    }

    fn cast_error(sqlite_err: SqliteError) -> Error {
        Error::new(ErrorKind::Interrupted, sqlite_err.message)
    }

    fn query<F>(&self, query: &'static str, closure: F) -> Result<SqliteRows, Error> {
        let mut stmt = self.db.prepare(query).unwrap();
        let mut rows = stmt.query_map(&[], closure).unwrap();

        Ok(rows)
    }

    fn get_build(&self) -> Result<i32, Error> {
        let mut rows = try!(self.query("SELECT build, created, last_run FROM schema", |row| {
            StoreSchema {
                build: row.get(0), /* created: row.get(1),
                                    * last_run: row.get(2), */
            }
        }));

        let result = match rows.next() {
            Some(r) => r.unwrap().get(0),
            None => return Err(Error::new(ErrorKind::NotFound, "Schema not found.")),
        };

        Ok(result)
    }
}

#[derive(Debug)]
struct Trend {
    name: String,
    url: String,
    description: String,
    date: DateTime<UTC>,
}

impl<'a> From<&'a Value> for Trend {
    fn from(data: &Value) -> Self {
        Trend {
            name: str_to_string(data.find("full_name").unwrap().as_string().unwrap()),
            url: str_to_string(data.find("html_url").unwrap().as_string().unwrap()),
            description: str_to_string(data.find("description").unwrap().as_string().unwrap()),
            date: UTC::now(),
        }
    }
}

struct Tweet {
    message: String,
}

impl Into<Tweet> for Trend {
    fn into(self) -> Tweet {
        Tweet { message: format!("{} - {} {}", self.name, self.description, self.url) }
    }
}

fn fetch_trends() -> Value {
    let url = format!("{}{}{}",
                      GITHUB_API_URL,
                      "/search/repositories",
                      "?q=language:rust&sort=stars&order=desc");

    let client = Client::new();
    let mut headers = Headers::new();
    headers.set(UserAgent("rust/rust-trends-bot".to_owned()));

    let mut response = match client.get(&*url).headers(headers).send() {
        Ok(resp) => resp,
        Err(_) => panic!("Failed to fetch!"),
    };

    let mut buffer = String::new();
    response.read_to_string(&mut buffer);

    serde_json::from_str(&*buffer).unwrap()
}

fn string_to_json(s: &String) -> Value {
    serde_json::from_str(&*s).unwrap()
}

fn str_to_string(s: &str) -> String {
    String::from(s)
}

fn temp_fetch_trends() -> Value {
    let mut file = File::open("trends.json").unwrap();
    let mut buffer = String::new();
    file.read_to_string(&mut buffer).unwrap();

    string_to_json(&buffer)
}

fn main() {
    let store = Store::new();
    let json = temp_fetch_trends();
    let items = json.find("items").unwrap();

    for trend in items.as_array().unwrap() {
        let ts = Trend::from(trend);
        let t: Tweet = ts.into();
        println!("{:?}", t.message);
    }
}
