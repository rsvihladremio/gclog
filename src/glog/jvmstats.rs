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

use crate::human::human_bytes_base_1k;
pub struct MemoryStats {
    pub physical_memory_str: String,
    pub physical_memory_bytes: i64,
}

pub fn parse_memory(line: String) -> MemoryStats {
    //Memory: 4k page, physical 128000000k(127996468k free), swap 0k(0k free)
    let tokens: Vec<&str> = line.split(' ').collect();
    let physical_raw: &str = tokens[4];
    let tokens_for_ram: Vec<&str> = physical_raw.split('(').collect();
    let total_ram_string = tokens_for_ram[0].trim_end_matches('k');
    let total_ram_k = total_ram_string.parse::<i64>().unwrap();
    let total_ram_bytes = total_ram_k * 1000;
    let total_ram = human_bytes_base_1k(total_ram_bytes);
    MemoryStats {
        physical_memory_str: format!("Total System RAM:    {total_ram}"),
        physical_memory_bytes: total_ram_bytes,
    }
}

pub fn parse_jdk_stats(line: String) -> String {
    //OpenJDK 64-Bit Server VM (25.332-b09) for linux-amd64 JRE (1.8.0_332-b09), built on Apr 20 2022 08:18:57 by "openjdk" with gcc 4.4.7 20120313 (Red Hat 4.4.7-23)
    line
}

#[cfg(test)]
mod tests {
    use super::parse_memory;

    #[test]
    fn test_parse_memory() {
        let line =
            "Memory: 4k page, physical 128000000k(127996468k free), swap 0k(0k free)".to_string();
        let result = parse_memory(line);
        assert_eq!(result.physical_memory_str, "Total System RAM:    128.00 gb");
        assert_eq!(result.physical_memory_bytes, 128000000 * 1000)
    }
}
