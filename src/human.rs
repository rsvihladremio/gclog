// Copyright 2022 Dremio
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use time::{format_description, OffsetDateTime};

const SECOND: i64 = 1000;
const MINUTE: i64 = SECOND * 60;
const HOUR: i64 = MINUTE * 60;
const DAY: i64 = HOUR * 24;

pub fn human_duration(duration_millis: i64) -> String {
    if duration_millis > DAY {
        format!("{:.2} days", duration_millis as f64 / DAY as f64)
    } else if duration_millis > HOUR {
        format!("{:.2} hours", duration_millis as f64 / HOUR as f64)
    } else if duration_millis > MINUTE {
        format!("{:.2} minutes", duration_millis as f64 / MINUTE as f64)
    } else if duration_millis > SECOND {
        format!("{:.2} seconds", duration_millis as f64 / SECOND as f64)
    } else {
        format!("{duration_millis:.2} milliseconds")
    }
}

pub fn human_time(ts_milli: i64) -> String {
    let ts = ts_milli / 1000;
    let naive = OffsetDateTime::from_unix_timestamp(ts);
    if let Ok(result) = naive {
        let format =
            format_description::parse("[year]-[month]-[day]T[hour]:[minute]:[second].[subsecond]Z")
                .expect("unable to setup format");
        result.format(&format).expect("unable to handle format")
    } else {
        "".to_string()
    }
}

pub fn human_percentage(perc: f64) -> String {
    if perc.is_nan() {
        return "0.00%".to_string();
    }
    format!("{:.2}%", perc * 100.00)
}

pub fn human_bytes_i128(bytes: i128) -> String {
    if bytes > 1024 * 1024 * 1024 * 1024 {
        return format!(
            "{:.2} tb",
            bytes as f64 / (1024.0 * 1024.0 * 1024.0 * 1024.0)
        );
    } else if bytes > 1024 * 1024 * 1024 {
        return format!("{:.2} gb", bytes as f64 / (1024.0 * 1024.0 * 1024.0));
    } else if bytes > 1024 * 1024 {
        return format!("{:.2} mb", bytes as f64 / (1024.0 * 1024.0));
    } else if bytes > 1024 {
        return format!("{:.2} kb", bytes as f64 / 1024.0);
    }
    format!("{bytes:.2} bytes")
}

pub fn human_bytes(bytes: i64) -> String {
    if bytes > 1024 * 1024 * 1024 * 1024 {
        return format!(
            "{:.2} tb",
            bytes as f64 / (1024.0 * 1024.0 * 1024.0 * 1024.0)
        );
    } else if bytes > 1024 * 1024 * 1024 {
        return format!("{:.2} gb", bytes as f64 / (1024.0 * 1024.0 * 1024.0));
    } else if bytes > 1024 * 1024 {
        return format!("{:.2} mb", bytes as f64 / (1024.0 * 1024.0));
    } else if bytes > 1024 {
        return format!("{:.2} kb", bytes as f64 / 1024.0);
    }
    format!("{bytes:.2} bytes")
}

pub fn human_bytes_base_1k(bytes: i64) -> String {
    let base = 1000.0;
    let base_i64 = 1000;
    if bytes > base_i64 * base_i64 * base_i64 * base_i64 {
        return format!("{:.2} tb", bytes as f64 / (base * base * base * base));
    } else if bytes > base_i64 * base_i64 * base_i64 {
        let gb = bytes as f64 / (base * base * base);
        return format!("{gb:.2} gb");
    } else if bytes > base_i64 * base_i64 {
        return format!("{:.2} mb", bytes as f64 / (base * base));
    } else if bytes > base_i64 {
        return format!("{:.2} kb", bytes as f64 / base);
    }
    format!("{bytes:.2} bytes")
}

pub fn human_metric(metric_name: String, value: i64) -> String {
    if metric_name.ends_with("BYTES_READ") {
        return human_bytes(value);
    } else if metric_name.ends_with("_NS") {
        return human_duration(value / 1000000);
    }
    "".to_string()
}
