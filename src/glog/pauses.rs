use std::fmt;
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
use std::str::FromStr;
use std::{collections::HashMap, error::Error};

use histogram::Histogram;
use tabled::object::Columns;
use tabled::{Alignment, Modify, Table, Tabled};
use time::{format_description, OffsetDateTime};

use crate::human::{human_duration, human_time};

pub struct GCPause {
    pub is_full_gc: bool,
    pub attributes: Vec<String>,
    pub gc_type: String,
    pub pause_time_seconds: f64,
    pub time_epoch: i64,
    pub heap_sizing: HeapSizing,
}
#[derive(Debug, PartialEq, Eq)]
pub enum HeapSizing {
    None,
    Expansion,
    Shrinking,
}

pub fn has_gc(line: &str) -> bool {
    line.contains("GC pause") || line.contains("GC (") || line.contains("Full GC ")
}

pub fn parse_full_gc_pause(multiline: String) -> Result<GCPause, Box<dyn Error>> {
    //super duper lazy way to do this

    let tokens: Vec<&str> = multiline.split('\n').collect();
    let index_with_gc = tokens.iter().position(|x| has_gc(x));
    let mut pause = parse_gc_pause((&tokens[index_with_gc.unwrap_or_default()]).to_string())?;
    let mut new_attributes = vec![];
    for attr in pause.attributes {
        if attr == "before full gc" {
            continue;
        }
        new_attributes.push(attr);
    }
    pause.attributes = new_attributes;

    let mut total_open = 0;
    let mut total_closed = 0;
    let mut seconds = 0.0;
    let heap_sizing = get_gc_resizing(&multiline);
    let mut skip = false;
    for line in tokens {
        if skip {
            if line.starts_with("Total") {
                skip = false;
            }
            continue;
        }
        total_closed += line.matches(']').count();
        total_open += line.matches('[').count();
        if line.contains("Histogram") {
            skip = true;
        }
        if total_open - total_closed != 0 {
            continue;
        }

        if total_open - total_closed == 0 && line.ends_with(" secs]") {
            //let mut seconds_start = false;
            //let mut seconds_str = "".to_string();
            let tokens = line.split(" secs]");
            let filtered: Vec<&str> = tokens.filter(|x| !x.is_empty()).collect();
            let seconds_str_raw = filtered
                .last()
                .expect("no last element")
                .split(", ")
                .collect::<Vec<&str>>();
            let seconds_str = seconds_str_raw.last().expect("unable to find last element");
            // for c in line.chars() {
            //     if c == ',' {
            //         seconds_start = true;
            //         continue;
            //     }
            //     if seconds_start {
            //         if c == ' ' && !seconds_str.is_empty() {
            //             break;
            //         }
            //         seconds_str.push(c);
            //     }
            // }
            seconds = f64::from_str(seconds_str.trim()).unwrap();
            break;
        }
    }

    Ok(GCPause {
        attributes: pause.attributes,
        gc_type: pause.gc_type,
        pause_time_seconds: seconds,
        time_epoch: pause.time_epoch,
        is_full_gc: multiline.contains("Full GC"),
        heap_sizing,
    })
}

#[derive(Debug)]
pub struct SecondsParseError {
    pub seconds: String,
    pub line: String,
}

impl Error for SecondsParseError {}

impl fmt::Display for SecondsParseError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "unable to seconds string of {} with line of '{}'",
            &self.seconds, &self.line
        )
    }
}
#[derive(Debug)]
pub struct ParseError {
    pub datetime_str: String,
}

impl Error for ParseError {}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "unable to parse date time string of {}",
            &self.datetime_str
        )
    }
}
fn get_epoch(datetime_line: String) -> Result<i64, ParseError> {
    let format = format_description::parse(
        "[year]-[month]-[day]T[hour]:[minute]:[second].[subsecond digits:3][offset_hour sign:mandatory][offset_minute]",
    ).unwrap();
    let time_raw = OffsetDateTime::parse(&datetime_line, &format);
    match time_raw {
        Ok(date) => Ok(date.unix_timestamp()),
        Err(_) => Err(ParseError {
            datetime_str: datetime_line,
        }),
    }
}

pub fn get_gc_resizing(line: &str) -> HeapSizing {
    if line.contains("calculated expansion amount: 0 bytes")
        || line.contains("(Heap Sizing) did not expand the heap")
    {
        HeapSizing::None
    } else if line.contains("(Heap Sizing) expand the heap")
        || line.contains("(Heap Sizing) attempt heap expansion")
    {
        HeapSizing::Expansion
    } else if line.contains("(Heap Sizing) shrink the heap") {
        HeapSizing::Shrinking
    } else {
        HeapSizing::None
    }
}

pub fn parse_gc_pause(line: String) -> Result<GCPause, Box<dyn Error>> {
    //2021-02-22T01:01:02.120+0000: 22000.498: [GC pause (G1 Humongous Allocation) (young) (initial-mark), 0.0911111 secs]
    //2021-02-22T01:01:02.120+0000: 22000.498: [GC pause (G1 Evacuation Pause) (young), 0.0110001 secs]
    //2021-02-22T01:01:02.120+0000: 22000.498: [GC pause (G1 Humongous Allocation) (young) (to-space exhausted), 1.9384781 secs]
    //2021-02-22T01:01:02.120+0000: 22000.498: [GC pause (G1 Evacuation Pause) (mixed), 0.0111111 secs]
    //2021-02-22T01:01:02.120+0000: 22000.498: [GC pause (GCLocker Initiated GC) (young), 0.0111111 secs]
    //19999.636: [Full GC (Ergonomics)  880219K->614437K(270720K), 0.0111111 secs]
    //19999.766: [GC (Allocation Failure)  880219K->614437K(270720K), 0.0111111 secs]
    let mut attributes: Vec<String> = vec![];
    let mut gc_type: String = "".to_string();
    let mut pause_time_seconds: f64 = 0.00;
    let tokens: Vec<&str> = line.split(": ").collect();
    //get the string and convert it
    let (head, tail) = tokens.split_at(1);
    let datetime_line = head[0];
    //for now silently ignore this as we are not making good use of logs in any case.
    let time_epoch = get_epoch(datetime_line.to_string()).unwrap_or_default();
    let gc_pause = tail.join(": ");
    let mut is_full_gc = false;
    let mut gc_type_str = "".to_string();
    let mut start_looking_for_full_gc = false;

    let mut attribute_start = false;
    let mut attribute: String = "".to_string();
    let mut seconds_start: bool = false;
    let mut seconds_str: String = "".to_string();
    let heap_sizing = get_gc_resizing(&gc_pause);
    let mut open_brackets = 0;
    let mut closed_brackets = 0;
    for c in gc_pause.chars() {
        if c == '[' {
            open_brackets += 1;
            if gc_type_str.is_empty() {
                start_looking_for_full_gc = true;
            }
            continue;
        }
        if c == ']' {
            closed_brackets += 1;
            if closed_brackets == open_brackets {
                break;
            }
            continue;
        }
        if open_brackets - closed_brackets != 1 && open_brackets > 0 {
            continue;
        }
        if start_looking_for_full_gc {
            if c == ' ' && !gc_type_str.is_empty() {
                start_looking_for_full_gc = false;
                if gc_type_str == "Full" {
                    is_full_gc = true;
                }
                continue;
            }
            gc_type_str.push(c);
            continue;
        }
        if c == ',' {
            seconds_start = true;
            continue;
        }

        if c == '(' {
            attribute_start = true;
            continue;
        }
        if c == ')' {
            attribute_start = false;
            //skip attributes with numbers in them
            if !attribute.chars().any(char::is_numeric) || attribute.contains("G1") {
                attributes.push(attribute);
            }
            attribute = "".to_string();
            continue;
        }
        if attribute_start {
            attribute.push(c);
            continue;
        }
        if seconds_start {
            if c == ' ' {
                if !seconds_str.is_empty() {
                    break;
                }
                continue;
            }
            seconds_str.push(c);
            continue;
        }
    }
    if !attributes.is_empty() {
        if is_full_gc {
            gc_type = "Full GC".to_string();
        } else {
            let (head, tail) = attributes.split_at(1);
            gc_type = head[0].to_string();
            attributes = tail.to_vec();
        }
    }
    if !seconds_str.is_empty() {
        let result = f64::from_str(seconds_str.as_str());
        pause_time_seconds = match result {
            Ok(seconds) => seconds,
            Err(e) => {
                println!("{e}");
                println!(
                    "{}",
                    SecondsParseError {
                        seconds: seconds_str,
                        line,
                    }
                );
                0.0
            }
        }
    }
    Ok(GCPause {
        is_full_gc,
        attributes,
        gc_type,
        pause_time_seconds,
        time_epoch,
        heap_sizing,
    })
}

pub struct GCSummary {
    pub gc_name: String,
    pub total_pauses: i64,
    pub total_seconds_paused: f64,
    pub longest_pause_seconds: f64,
    pub shortest_pause_seconds: f64,
    pub pause_histo_millis: Histogram,
}

#[derive(Tabled)]
pub struct GCSummaryRow {
    #[tabled(rename = "GC")]
    pub gc_name: String,
    #[tabled(rename = "Total Pauses")]
    pub total_pauses: i64,
    #[tabled(display_with = "format_float", rename = "Total Pause Time")]
    pub total_seconds_paused: f64,
    #[tabled(display_with = "format_float", rename = "Min Pause")]
    pub shortest_pause_seconds: f64,
    #[tabled(display_with = "format_float", rename = "P50 Pause")]
    pub p50: f64,
    #[tabled(display_with = "format_float", rename = "P99 Pause")]
    pub p99: f64,
    #[tabled(display_with = "format_float", rename = "Max Pause")]
    pub longest_pause_seconds: f64,
}

fn format_float(float: &f64) -> String {
    format!("{float:.2}")
}

pub fn generate_gc_name(pause: &GCPause) -> String {
    let mut attrs = vec![];
    for attr in &pause.attributes {
        attrs.push(format!("({attr})"));
    }
    attrs.sort();
    if attrs.is_empty() {
        return pause.gc_type.to_string();
    }
    format!("{} - {}", pause.gc_type, attrs.join(""))
}
pub fn show_max_pause_times(pauses: &Vec<GCPause>) -> String {
    let mut max_pause_time: f64 = 0.0;
    let mut max_pause = None;
    for pause in pauses {
        if pause.pause_time_seconds > max_pause_time {
            max_pause_time = pause.pause_time_seconds;
            max_pause = Some(pause);
        };
    }
    if max_pause.is_none() {
        "No Pauses".to_string()
    } else {
        format!(
            "Timestamp: {}\nPause Time {}\nPause Type {}",
            human_time(max_pause.unwrap().time_epoch * 1000),
            human_duration((max_pause.unwrap().pause_time_seconds * 1000.0) as i64),
            max_pause.unwrap().gc_type
        )
    }
}

pub fn generate_pause_table(pauses: &Vec<GCPause>) -> String {
    let mut pause_table: HashMap<String, GCSummary> = HashMap::new();
    for pause in pauses {
        let gc_name = generate_gc_name(pause);
        let mut maybe_new = Histogram::new();
        maybe_new
            .increment((pause.pause_time_seconds * 1000.0) as u64)
            .expect("unable to increment new histo.. that is silly");
        pause_table
            .entry(gc_name.to_string())
            .and_modify(|summary| {
                if pause.pause_time_seconds > summary.longest_pause_seconds {
                    summary.longest_pause_seconds = pause.pause_time_seconds;
                }
                if pause.pause_time_seconds < summary.shortest_pause_seconds {
                    summary.shortest_pause_seconds = pause.pause_time_seconds;
                }
                summary.total_pauses += 1;
                summary
                    .pause_histo_millis
                    .increment((pause.pause_time_seconds * 1000.0) as u64)
                    .unwrap();
                summary.total_seconds_paused += pause.pause_time_seconds;
            })
            .or_insert(GCSummary {
                gc_name,
                longest_pause_seconds: pause.pause_time_seconds,
                shortest_pause_seconds: pause.pause_time_seconds,
                total_pauses: 1,
                total_seconds_paused: pause.pause_time_seconds,
                pause_histo_millis: maybe_new,
            });
    }

    let mut gc_summaries: Vec<GCSummaryRow> = pause_table
        .iter()
        .map(|f| GCSummaryRow {
            gc_name: f.1.gc_name.to_string(),
            longest_pause_seconds: f.1.longest_pause_seconds,
            p50: f.1.pause_histo_millis.percentile(50.0).unwrap() as f64 / 1000.0,
            p99: f.1.pause_histo_millis.percentile(99.0).unwrap() as f64 / 1000.0,
            shortest_pause_seconds: f.1.shortest_pause_seconds,
            total_pauses: f.1.total_pauses,
            total_seconds_paused: f.1.total_seconds_paused,
        })
        .collect();
    gc_summaries.sort_by_key(|f| f.gc_name.to_string());
    Table::new(gc_summaries)
        .with(Modify::new(Columns::first()).with(Alignment::left()))
        .to_string()
}

#[cfg(test)]
mod tests {
    use crate::{
        glog::pauses::{parse_full_gc_pause, parse_gc_pause, GCPause, HeapSizing},
        tests::approx_equal_f64,
    };
    use std::vec;

    use super::{generate_gc_name, generate_pause_table};

    #[test]
    fn test_generate_gc_name() {
        let name = generate_gc_name(&GCPause {
            is_full_gc: false,
            attributes: vec!["to-space exhausted".to_string(), "young".to_string()],
            gc_type: "G1 Humongous Allocation".to_string(),
            pause_time_seconds: 1.2,
            time_epoch: 1658405469,
            heap_sizing: HeapSizing::None,
        });
        assert_eq!(
            name,
            "G1 Humongous Allocation - (to-space exhausted)(young)"
        );
    }

    #[test]
    fn test_generate_pause_table() {
        let pause1 = GCPause {
            is_full_gc: false,
            attributes: vec!["to-space exhausted".to_string(), "young".to_string()],
            gc_type: "G1 Humongous Allocation".to_string(),
            pause_time_seconds: 1.25,
            time_epoch: 1658405158,
            heap_sizing: HeapSizing::None,
        };
        let pause2 = GCPause {
            is_full_gc: false,
            attributes: vec!["young".to_string()],
            gc_type: "G1 Evacuation Pause".to_string(),
            pause_time_seconds: 2.75,
            time_epoch: 1658405169,
            heap_sizing: HeapSizing::None,
        };
        let pause3 = GCPause {
            is_full_gc: false,
            attributes: vec!["young".to_string(), "to-space exhausted".to_string()],
            gc_type: "G1 Evacuation Pause".to_string(),
            pause_time_seconds: 100.15,
            time_epoch: 1658405469,
            heap_sizing: HeapSizing::None,
        };
        let pause4 = GCPause {
            is_full_gc: false,
            attributes: vec!["young".to_string(), "to-space exhausted".to_string()],
            gc_type: "G1 Evacuation Pause".to_string(),
            pause_time_seconds: 0.15,
            time_epoch: 1658409469,
            heap_sizing: HeapSizing::None,
        };
        let pauses = vec![pause1, pause2, pause3, pause4];
        let output = generate_pause_table(&pauses);
        assert_eq!(output, "+-------------------------------------------------------+--------------+------------------+-----------+-----------+-----------+-----------+
| GC                                                    | Total Pauses | Total Pause Time | Min Pause | P50 Pause | P99 Pause | Max Pause |
+-------------------------------------------------------+--------------+------------------+-----------+-----------+-----------+-----------+
| G1 Evacuation Pause - (to-space exhausted)(young)     |      2       |      100.30      |   0.15    |  100.20   |  100.20   |  100.15   |
+-------------------------------------------------------+--------------+------------------+-----------+-----------+-----------+-----------+
| G1 Evacuation Pause - (young)                         |      1       |       2.75       |   2.75    |   2.75    |   2.75    |   2.75    |
+-------------------------------------------------------+--------------+------------------+-----------+-----------+-----------+-----------+
| G1 Humongous Allocation - (to-space exhausted)(young) |      1       |       1.25       |   1.25    |   1.25    |   1.25    |   1.25    |
+-------------------------------------------------------+--------------+------------------+-----------+-----------+-----------+-----------+
");
    }

    #[test]
    fn test_histogram_on_generate_pause_table() {
        let mut pauses = vec![];
        for i in 5..500 {
            pauses.push(GCPause {
                is_full_gc: false,
                attributes: vec!["young".to_string(), "to-space exhausted".to_string()],
                gc_type: "G1 Evacuation Pause".to_string(),
                pause_time_seconds: 0.15 + i as f64,
                time_epoch: 1658409469,
                heap_sizing: HeapSizing::None,
            });
        }
        let output = generate_pause_table(&pauses);
        assert_eq!(output, "+---------------------------------------------------+--------------+------------------+-----------+-----------+-----------+-----------+
| GC                                                | Total Pauses | Total Pause Time | Min Pause | P50 Pause | P99 Pause | Max Pause |
+---------------------------------------------------+--------------+------------------+-----------+-----------+-----------+-----------+
| G1 Evacuation Pause - (to-space exhausted)(young) |     495      |    124814.25     |   5.15    |  253.23   |  496.24   |  499.15   |
+---------------------------------------------------+--------------+------------------+-----------+-----------+-----------+-----------+
")
    }

    #[test]
    fn test_parse_gc_pause_with_lots_of_commas() {
        let line = "2022-01-02T11:11:01.111+0000: 54105.596: [GC pause (G1 Evacuation Pause) (young) 54105.596: [G1Ergonomics (CSet Construction) start choosing CSet, _pending_cards: 0, predicted base time: 8.15 ms, remaining time: 491.85 ms, target pause time: 500.00 ms]
54105.596: [G1Ergonomics (CSet Construction) add young regions to CSet, eden: 0 regions, survivors: 0 regions, predicted young region time: 0.00 ms]
54105.596: [G1Ergonomics (CSet Construction) finish choosing CSet, eden: 0 regions, survivors: 0 regions, old: 0 regions, predicted pause time: 8.15 ms, target pause time: 500.00 ms]
2022-01-02T11:11:01.111+0000: 54105.603: [SoftReference, 0 refs, 0.0000313 secs]2022-01-02T11:11:01.111+0000: 54105.603: [WeakReference, 0 refs, 0.0000043 secs]2022-01-02T11:11:01.111+0000: 54105.603: [FinalReference, 0 refs, 0.0000039 secs]2022-01-02T11:11:01.111+0000: 54105.603: [PhantomReference, 0 refs, 0 refs, 0.0000040 secs]2022-01-02T11:11:01.111+0000: 54105.603: [JNI Weak Reference, 0.0000749 secs] 54105.604: [G1Ergonomics (Heap Sizing) attempt heap expansion, reason: recent GC overhead higher than threshold after GC, recent GC overhead: 51.72 %, threshold: 10.00 %, uncommitted: 0 bytes, calculated expansion amount: 0 bytes (20.00 %)]
, 0.9900000 secs]";
        let result_raw = parse_gc_pause(line.to_string());
        let result = result_raw.unwrap();
        assert_eq!(result.pause_time_seconds, 0.9900000);
        assert_eq!(result.gc_type, "G1 Evacuation Pause");
        assert_eq!(result.attributes.len(), 1);
        assert_eq!(result.attributes[0], "young");
        assert_eq!(result.heap_sizing, HeapSizing::None);
    }

    #[test]
    fn test_parse_gc_pause() {
        let line = "2021-02-22T01:01:02.120+0000: 22000.498: [GC pause (G1 Humongous Allocation) (young) (initial-mark), 0.0911111 secs]";
        let result_raw = parse_gc_pause(line.to_string());
        let result = result_raw.unwrap();
        assert_eq!(result.pause_time_seconds, 0.0911111);
        assert_eq!(result.gc_type, "G1 Humongous Allocation");
        assert_eq!(result.attributes.len(), 2);
        assert_eq!(result.attributes[0], "young");
        assert_eq!(result.attributes[1], "initial-mark");
    }

    #[test]
    fn test_parse_gc_pause_without_datetime_stamp() {
        let line =
            "16142.766: [GC (Allocation Failure)  24639447K->13665474K(26456064K), 0.0911111 secs]";
        let result_raw = parse_gc_pause(line.to_string());
        let result = result_raw.unwrap();
        assert_eq!(result.pause_time_seconds, 0.0911111);
        assert_eq!(result.gc_type, "Allocation Failure");
        assert_eq!(result.attributes.len(), 0);
    }

    #[test]
    fn test_parse_full_gc_pause() {
        let full_gc_line = "2022-01-02T11:11:01.111+0000: 234567.120: [Full GC (Allocation Failure) 2022-01-02T11:11:01.111+0000: 234567.120: [Class Histogram (before full gc):
num     #instances         #bytes  class name
----------------------------------------------
    1:       9013468      888888888  org.apache.arrow.memory.ArrowBuf
    2:       4547890      523123575  io.netty.buffer.PooledUnsafeDirectByteBuf
    3:         53333      444444444  [I
    4:       4312460      667801141  org.apache.arrow.memory.NettyAllocationManager
    5:       4312460      214605720  org.apache.arrow.memory.BufferLedger
    6:        964740      186324576  [J
    7:       4312460      153226880  io.netty.buffer.UnsafeDirectLittleEndian
    8:       4312460      147067264  [Ljava.lang.Object;
    9:       4312460      127067024  org.apache.arrow.vector.VarCharVector
    10:       4312460      124471040  io.netty.util.Recycler$DefaultHandle
    11:       4312460      101939880  org.apache.arrow.vector.BitVector
    12:       4312460       91111101  org.apache.arrow.memory.LowCostIdentityHashMap
    13:       4312460       91111101  com.dremio.exec.record.SimpleVectorWrapper
    14:       4312460       91111101  java.util.concurrent.atomic.AtomicInteger
    15:        431460       91111101  io.netty.buffer.PoolSubpage
    16:        431460       91111101  [C
    17:       4312460       91111101  org.apache.arrow.vector.complex.impl.VarCharReaderImpl
    18:       4312460       91111101  org.apache.arrow.vector.complex.impl.BitReaderImpl
    19:        431460       91111101  org.apache.arrow.vector.BigIntVector
    20:        431460       91111101  [B
    10821:             1             16  sun.util.resources.LocaleData$LocaleDataResourceBundleControl
Total      99999999     8643256886
, 1.1111111 secs]
    4185M->1198M(5336M), 4.3449655 secs]
        [Eden: 0.0B(724.0M)->0.0B(6114.0M) Survivors: 0.0B->0.0B Heap: 8185.4M(8192.0M)->1111.0M(5335.0M)], [Metaspace: 111001K->111110K(1111704K)]
";
        let result_raw = parse_full_gc_pause(full_gc_line.to_string());
        let result = result_raw.unwrap();
        assert_eq!(result.gc_type, "Full GC");
        assert!(
            approx_equal_f64(result.pause_time_seconds, 4.34, 0.01),
            "expected {} but was {}",
            4.34,
            result.pause_time_seconds
        );
    }
}
