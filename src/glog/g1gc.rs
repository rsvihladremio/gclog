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

use super::convert::convert_bytes_to_mb;

pub fn get_g1_target_millis(gc_flags: &Vec<String>) -> i32 {
    for flag in gc_flags {
        if flag.starts_with("-XX:MaxGCPauseMillis=") {
            let o = flag
                .strip_prefix("-XX:MaxGCPauseMillis=")
                .expect("unable to remove -XX:MaxGCPauseMillis= prefix");
            return o.parse().expect("unable to parse string");
        }
    }
    //default sourced from https://www.oracle.com/technical-resources/articles/java/g1gc.html
    200
}
pub fn get_g1_gc_region_size_mb(min_heap_gb: f32, gc_flags: &Vec<String>) -> f32 {
    for flag in gc_flags {
        if flag.starts_with("-XX:G1HeapRegionSize=") {
            let o = flag
                .strip_prefix("-XX:G1HeapRegionSize=")
                .expect("unable to remove -XX:G1HeapRegionSize= prefix");
            let region_size = o.parse().expect("unable to parse string");
            return convert_bytes_to_mb(region_size);
        }
    }
    get_region_for_heap(min_heap_gb)
}

// get_region_for_heap is sourced from the table located here https://stackoverflow.com/questions/46786601/how-to-know-region-size-used-of-g1-garbage-collector
// G1 region-size in Java-8 is based on startingHeapSize/2048 and rounded DOWN to the first power of 2 between 1MB and 32MB; region sizes <1MB or >32MB are not supported.
//
// you can also set the region-size via -XX:G1HeapRegionSize=n (note, the value has the same power-of-2/range restrictions).
//
// so actually the JVM seems biased towards a region count between 2048 and 4095 (assuming a heap between 2GB and 128GB).
//
// in general these are the region-sizes per heap-size range:
//
//  <4GB -  1MB
//  <8GB -  2MB
//  <16GB -  4MB
//  <32GB -  8MB
//  <64GB - 16MB
//  64GB+ - 32MB
// note, MB is actually MiB and GB is actually GiB
fn get_region_for_heap(min_heap_gb: f32) -> f32 {
    //let min_region_size = 1.0;
    //let max_region_size = 32.0;
    let heap_bytes = min_heap_gb * 1024.0 * 1024.0 * 1024.0;
    let region_size = heap_bytes / 2048.0;

    let region_size_in_mb = region_size / (1024.0 * 1024.0);
    if region_size_in_mb < 2.0 {
        return 1.0;
    } else if region_size_in_mb > 2.0 && region_size_in_mb < 4.0 {
        return 2.0;
    } else if region_size_in_mb > 4.0 && region_size_in_mb < 8.0 {
        return 4.0;
    } else if region_size_in_mb > 8.0 && region_size_in_mb < 16.0 {
        return 8.0;
    } else if region_size_in_mb > 16.0 && region_size_in_mb < 32.0 {
        return 16.0;
    }
    32.0
}

#[cfg(test)]
mod tests {
    use std::vec;

    use crate::{
        glog::g1gc::{get_g1_gc_region_size_mb, get_g1_target_millis, get_region_for_heap},
        tests::assert_approx_equal,
    };

    #[test]
    fn test_get_target_millis() {
        let default_target_millis = get_g1_target_millis(&vec!["".to_string()]);
        assert_eq!(default_target_millis, 200);
        let target_millis_with_flag = get_g1_target_millis(&vec![
            "".to_string(),
            "-XX:MaxGCPauseMillis=500".to_string(),
            "".to_string(),
        ]);
        assert_eq!(target_millis_with_flag, 500);
    }

    #[test]
    fn test_get_g1_gc_region_size_mb() {
        let region_size = get_g1_gc_region_size_mb(
            0.0,
            &vec!["-XX:G1HeapRegionSize=33554432".to_string(), "".to_string()],
        );
        assert_approx_equal(region_size, 32.0, 0.01);
    }

    #[test]
    fn test_get_g1_gc_region_size_mb_impossible_size() {
        //even though this is an entirely impossible size to get for the g1gc I am adding this as potential possibility and not
        //rounding it down. I am choosing to allow this to happen
        let region_size = get_g1_gc_region_size_mb(
            0.0,
            &vec!["-XX:G1HeapRegionSize=32000000".to_string(), "".to_string()],
        );
        assert_approx_equal(region_size, 30.51, 0.01);
    }

    #[test]
    fn test_get_g1_gc_region_size_mb_with_no_region_size() {
        let region_size = get_g1_gc_region_size_mb(64.0, &vec!["".to_string(), "".to_string()]);
        assert_approx_equal(region_size, 32.0, 0.01);
    }

    #[test]
    fn test_default_g1gc_region_size() {
        assert_approx_equal(get_region_for_heap(1.0), 1.0, 0.01);
        assert_approx_equal(get_region_for_heap(4.01), 2.0, 0.01);
        assert_approx_equal(get_region_for_heap(8.01), 4.0, 0.01);
        assert_approx_equal(get_region_for_heap(16.01), 8.0, 0.01);
        assert_approx_equal(get_region_for_heap(32.01), 16.0, 0.01);
        assert_approx_equal(get_region_for_heap(64.01), 32.0, 0.01);
        assert_approx_equal(get_region_for_heap(128.01), 32.0, 0.01);
    }
}
