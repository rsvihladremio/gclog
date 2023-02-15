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

use std::fmt;

use super::{
    convert::convert_bytes_to_gb,
    g1gc::{get_g1_gc_region_size_mb, get_g1_target_millis},
};

pub struct GCFlags {
    pub collector: Collector,
    pub max_heap_size_gb: f32,
    pub min_heap_size_gb: f32,
    pub region_size_mb: f32,
    pub target_pause_millis: i32,
    pub max_direct_memory_gb: f32,
    pub all_flags: Vec<String>,
}

impl fmt::Display for GCFlags {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.collector == Collector::G1GC {
            write!(
                f,
                "collector:           {:?}
target pause millis: {}
region size:         {:.2} mb
max heap:            {:.2} gb
initial heap:        {:.2} gb
max direct memory:   {:.2} gb
flags:\n{}",
                self.collector,
                self.target_pause_millis,
                self.region_size_mb,
                self.max_heap_size_gb,
                self.min_heap_size_gb,
                self.max_direct_memory_gb,
                &self.all_flags.join("\n")
            )
        } else {
            write!(
                f,
                "collector:           {:?}
max heap:            {:.2} gb
initial heap:        {:.2} gb
max direct memory:   {:.2} gb
flags:\n{}",
                self.collector,
                self.max_heap_size_gb,
                self.min_heap_size_gb,
                self.max_direct_memory_gb,
                &self.all_flags.join("\n")
            )
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum Collector {
    SerialGC,   //-XX:+UseSerialGC
    G1GC,       //-XX:+UseG1GC
    CMS,        //-XX:+UseParNewGC
    Parallel,   //-XX:+UseParallelGC
    ZGC,        //-XX:+UseZGC
    Shenandoah, //-XX:+UseShenandoahGC
    Unknown,
}

fn get_collector(gc_flags: &Vec<String>) -> Collector {
    for flag in gc_flags {
        if flag == "-XX:+UseZGC" {
            return Collector::ZGC;
        } else if flag == "-XX:+UseG1GC" {
            return Collector::G1GC;
        } else if flag == "-XX:+UseShenandoahGC" {
            return Collector::Shenandoah;
        } else if flag == "-XX:+UseParallelGC" {
            return Collector::Parallel;
        } else if flag == "-XX:+UseParNewGC" {
            return Collector::CMS;
        } else if flag == "-XX:+UseSerialGC" {
            return Collector::SerialGC;
        }
    }
    Collector::Unknown
}

fn get_min_heap_size_gb(gc_flags: &Vec<String>, physical_memory_bytes: i64) -> f32 {
    for flag in gc_flags {
        if flag.starts_with("-XX:InitialHeapSize=") {
            let o = flag
                .strip_prefix("-XX:InitialHeapSize=")
                .expect("unable to remove -XX:InitialHeapSize= prefix");
            let min_heap_bytes = o.parse().expect("unable to parse string");
            return convert_bytes_to_gb(min_heap_bytes);
        }
    }
    // source https://www.oracle.com/java/technologies/javase/6u18.html
    // * The maximum heap size is not actually used by the JVM unless your program
    // * creates enough objects to require it. A much smaller amount, termed the initial
    // * heap size, is allocated during JVM initialization. This amount is at least 8
    // * megabytes and otherwise 1/64 of physical memory up to a physical memory size of 1 gigabyte.
    //
    let expected_min_heap_bytes = physical_memory_bytes as f64 * 0.015625; // = 1/64
    let eight_mb_in_bytes = 1024.0 * 1024.0 * 8.0;
    if expected_min_heap_bytes < eight_mb_in_bytes {
        return (eight_mb_in_bytes / 1024.0 / 1024.0 / 1024.0) as f32; //8mb in gb
    }
    (expected_min_heap_bytes / 1024.0 / 1024.0 / 1024.0) as f32
}

fn get_max_heap_size_gb(gc_flags: &Vec<String>, physical_memory_bytes: i64) -> f32 {
    for flag in gc_flags {
        if flag.starts_with("-XX:MaxHeapSize=") {
            let o = flag
                .strip_prefix("-XX:MaxHeapSize=")
                .expect("unable to remove -XX:MaxHeapSize= prefix");
            let max_heap_bytes = o.parse().expect("unable to parse string");
            return convert_bytes_to_gb(max_heap_bytes);
        }
    }
    let expected_max_heap = physical_memory_bytes as f64 * 0.25;
    //if less than 1gb return that https://www.oracle.com/java/technologies/javase/6u18.html
    // * The default maximum heap size is half of the physical memory up to a
    // * physical memory size of 192 megabytes and otherwise one fourth of the
    // * physical memory up to a physical memory size of 1 gigabyte. For example,
    // * if your machine has 128 megabytes of physical memory, then the maximum heap size is 64 megabytes,
    // *  and greater than or equal to 1 gigabyte of physical memory results in a maximum heap size of 256 megabytes.
    // NOTE: we are going to assume all servers have more than 192mb
    if expected_max_heap < (1024.0 * 1024.0 * 1024.0) {
        return 1.0;
    }
    (expected_max_heap / 1024.0 / 1024.0 / 1024.0) as f32
}

pub fn parse_gc_flags(line: String, physical_memory_bytes: i64) -> GCFlags {
    let mut all_flags: Vec<String> = vec![];
    //CommandLine flags: -XX:+DisableExplicitGC -XX:ErrorFile=/opt/dremio/data/hs_err_pid%p.log -XX:G1HeapRegionSize=33554432 -XX:GCLogFileSize=4096000 -XX:+HeapDumpOnOutOfMemoryError -XX:HeapDumpPath=/opt/dremio/data/ -XX:InitialHeapSize=2048000000 -XX:InitiatingHeapOccupancyPercent=25 -XX:MaxDirectMemorySize=120259084288 -XX:MaxGCPauseMillis=500 -XX:MaxHeapSize=17179869184 -XX:NumberOfGCLogFiles=5 -XX:+PrintClassHistogramAfterFullGC -XX:+PrintClassHistogramBeforeFullGC -XX:+PrintGC -XX:+PrintGCDateStamps -XX:+PrintGCDetails -XX:+PrintGCTimeStamps -XX:+UseCompressedClassPointers -XX:+UseCompressedOops -XX:+UseG1GC -XX:+UseGCLogFileRotation
    let tokens = line.split(' ');
    let mut counter = 0;
    for t in tokens {
        if counter > 1 {
            all_flags.push(t.to_string());
            continue;
        }
        counter += 1
    }
    let collector = get_collector(&all_flags);
    let max_heap_size_gb = get_max_heap_size_gb(&all_flags, physical_memory_bytes);
    let min_heap_size_gb = get_min_heap_size_gb(&all_flags, physical_memory_bytes);
    let max_direct_memory_gb = get_max_direct_memory_gb(&all_flags, max_heap_size_gb);
    let mut target_pause_millis = 0;
    let mut region_size_mb = 0.0;
    if collector == Collector::G1GC {
        region_size_mb = get_g1_gc_region_size_mb(max_heap_size_gb, &all_flags);
        target_pause_millis = get_g1_target_millis(&all_flags);
    }

    GCFlags {
        collector,
        max_heap_size_gb,
        min_heap_size_gb,
        target_pause_millis,
        region_size_mb,
        max_direct_memory_gb,
        all_flags,
    }
}

fn get_max_direct_memory_gb(gc_flags: &Vec<String>, max_heap_gb: f32) -> f32 {
    let expected_flag_name = "-XX:MaxDirectMemorySize=";
    for flag in gc_flags {
        if flag.starts_with(expected_flag_name) {
            let err_msg = format!("unable to remove {expected_flag_name} prefix");
            let o = flag
                .strip_prefix(expected_flag_name)
                .unwrap_or(err_msg.as_str());
            let raw_bytes: i64 = o.parse().expect("unable to parse string");
            return (raw_bytes as f64 / 1024.0 / 1024.0 / 1024.0) as f32;
        }
    }
    //default sourced from https://stackoverflow.com/questions/3773775/default-for-xxmaxdirectmemorysize
    //under jdk8
    // * .. but then the directMemory is set to maxMemory() ~= Heapsize here (if the maxDirectMemorySize-Parameter is not set):
    // *
    // *(from: https://github.com/frohoff/jdk8u-dev-jdk/blob/master/src/share/classes/sun/misc/VM.java#L286 )
    // *
    // * // Set the maximum amount of direct memory.  This value is controlled
    // * // by the vm option -XX:MaxDirectMemorySize=<size>.
    // * // The maximum amount of allocatable direct buffer memory (in bytes)
    // * // from the system property sun.nio.MaxDirectMemorySize set by the VM.
    // * // The system property will be removed.
    // * String s = (String)props.remove("sun.nio.MaxDirectMemorySize");
    // * if (s != null) {
    // *    if (s.equals("-1")) {
    // *        // -XX:MaxDirectMemorySize not given, take default
    // *        directMemory = Runtime.getRuntime().maxMemory();
    // *    } else {
    // *        long l = Long.parseLong(s);
    // *    if (l > -1)
    // *         directMemory = l;
    // *    }
    // * }
    max_heap_gb
}

#[cfg(test)]
mod tests {
    use crate::{
        glog::flags::{get_max_heap_size_gb, get_min_heap_size_gb, parse_gc_flags, Collector},
        tests::{approx_equal, assert_approx_equal},
    };

    use super::{get_collector, get_max_direct_memory_gb, GCFlags};

    #[test]
    fn test_get_min_heap() {
        let gc_flags = vec!["-XX:InitialHeapSize=64424509440".to_string()];
        let min_heap_size_gb = get_min_heap_size_gb(&gc_flags, 0);
        assert_approx_equal(min_heap_size_gb, 60.0, 0.01);
    }

    #[test]
    fn test_default_get_min_heap() {
        let gc_flags = vec![];
        let min_heap_size_gb = get_min_heap_size_gb(&gc_flags, 64 * 1024 * 1024 * 1024);
        assert_approx_equal(min_heap_size_gb, 1.0, 0.01);
    }

    #[test]
    fn test_default_get_min_heap_when_physical_memory_is_below_512mb() {
        let gc_flags = vec![];
        let min_heap_size_gb = get_min_heap_size_gb(&gc_flags, 256 * 1024 * 1024);
        assert_approx_equal(min_heap_size_gb, 8.0 / 1024.0, 0.01);
    }

    #[test]
    fn test_get_max_heap() {
        let gc_flags = vec!["-XX:MaxHeapSize=64424509440".to_string()];
        let max_heap_size_gb = get_max_heap_size_gb(&gc_flags, 0);
        assert_approx_equal(max_heap_size_gb, 60.0, 0.01);
    }

    #[test]
    fn test_default_get_max_heap() {
        let gc_flags = vec![];
        let max_heap_size_gb = get_max_heap_size_gb(&gc_flags, 240 * 1024 * 1024 * 1024);
        assert_approx_equal(max_heap_size_gb, 60.0, 0.01);
    }
    #[test]
    fn test_default_get_max_heap_when_physical_memory_is_below_4gb() {
        let gc_flags = vec![];
        let max_heap_size_gb = get_max_heap_size_gb(&gc_flags, 2 * 1024 * 1024 * 1024);
        assert_approx_equal(max_heap_size_gb, 1.0, 0.01);
    }

    #[test]
    fn test_get_max_direct_memory() {
        let gc_flags = vec!["-XX:MaxDirectMemorySize=64424509440".to_string()];
        let max_direct_memory = get_max_direct_memory_gb(&gc_flags, 32.0);
        assert_approx_equal(max_direct_memory, 60.0, 0.01);
    }

    #[test]
    fn test_get_default_max_direct_memory() {
        let gc_flags = vec![];
        let max_direct_memory = get_max_direct_memory_gb(&gc_flags, 32.0);
        assert!(approx_equal(max_direct_memory, 32.0, 0.01));
    }

    #[test]
    fn test_print_gc_flags() {
        let all_flags = vec![
            "-XX:G1HeapRegionSize=33554432".to_string(),
            "-XX:+HeapDumpOnOutOfMemoryError".to_string(),
            "-XX:HeapDumpPath=/var/log/dremio".to_string(),
            "-XX:InitialHeapSize=1073741824".to_string(),
            "-XX:InitiatingHeapOccupancyPercent=25".to_string(),
            "-XX:MaxDirectMemorySize=64424509440".to_string(),
            "-XX:MaxGCPauseMillis=500".to_string(),
            "-XX:MaxHeapSize=34359738368".to_string(),
            "-XX:+PrintClassHistogramAfterFullGC".to_string(),
            "-XX:+PrintClassHistogramBeforeFullGC".to_string(),
            "-XX:+PrintGC".to_string(),
            "-XX:+PrintGCDateStamps".to_string(),
            "-XX:+PrintGCDetails".to_string(),
            "-XX:+PrintGCTimeStamps".to_string(),
            "-XX:+UseCompressedClassPointers".to_string(),
            "-XX:+UseCompressedOops".to_string(),
            "-XX:+UseG1GC".to_string(),
        ];
        let flags = GCFlags {
            collector: Collector::G1GC,
            max_heap_size_gb: 10.0,
            min_heap_size_gb: 32.0,
            region_size_mb: 4.0,
            target_pause_millis: 200,
            max_direct_memory_gb: 60.0,
            all_flags,
        };
        let expected = "collector:           G1GC
target pause millis: 200
region size:         4.00 mb
max heap:            10.00 gb
initial heap:        32.00 gb
max direct memory:   60.00 gb
flags:
-XX:G1HeapRegionSize=33554432
-XX:+HeapDumpOnOutOfMemoryError
-XX:HeapDumpPath=/var/log/dremio
-XX:InitialHeapSize=1073741824
-XX:InitiatingHeapOccupancyPercent=25
-XX:MaxDirectMemorySize=64424509440
-XX:MaxGCPauseMillis=500
-XX:MaxHeapSize=34359738368
-XX:+PrintClassHistogramAfterFullGC
-XX:+PrintClassHistogramBeforeFullGC
-XX:+PrintGC
-XX:+PrintGCDateStamps
-XX:+PrintGCDetails
-XX:+PrintGCTimeStamps
-XX:+UseCompressedClassPointers
-XX:+UseCompressedOops
-XX:+UseG1GC";
        assert_eq!(flags.to_string(), expected.to_string());
    }

    #[test]
    fn test_print_gc_flags_non_g1gc() {
        let all_flags = vec![
            "-XX:CICompilerCount=4".to_string(),
            "-XX:ErrorFile=/var/log/dremio/hs_err_pid%p.log".to_string(),
            "-XX:GCLogFileSize=4096000".to_string(),
            "-XX:+HeapDumpOnOutOfMemoryError".to_string(),
            "-XX:HeapDumpPath=/var/log/dremio".to_string(),
            "-XX:InitialHeapSize=1073741824".to_string(),
            "-XX:InitiatingHeapOccupancyPercent=25".to_string(),
            "-XX:MaxDirectMemorySize=64424509440".to_string(),
            "-XX:MaxHeapSize=64424509440".to_string(),
            "-XX:MaxNewSize=20971520000".to_string(),
            "-XX:MinHeapDeltaBytes=524288".to_string(),
            "-XX:NewSize=703594496".to_string(),
            "-XX:NumberOfGCLogFiles=5".to_string(),
            "-XX:OldSize=1408237568".to_string(),
            "-XX:+PrintClassHistogramAfterFullGC".to_string(),
            "-XX:+PrintClassHistogramBeforeFullGC".to_string(),
            "-XX:+PrintGC".to_string(),
            "-XX:+PrintGCTimeStamps".to_string(),
            "-XX:+UseFastUnorderedTimeStamps".to_string(),
            "-XX:+UseGCLogFileRotation".to_string(),
            "-XX:+UseParallelGC".to_string(),
        ];
        let flags = GCFlags {
            collector: Collector::Parallel,
            max_heap_size_gb: 10.0,
            min_heap_size_gb: 32.0,
            region_size_mb: 0.0,
            target_pause_millis: 0,
            max_direct_memory_gb: 60.0,
            all_flags,
        };
        let expected = "collector:           Parallel
max heap:            10.00 gb
initial heap:        32.00 gb
max direct memory:   60.00 gb
flags:
-XX:CICompilerCount=4
-XX:ErrorFile=/var/log/dremio/hs_err_pid%p.log
-XX:GCLogFileSize=4096000
-XX:+HeapDumpOnOutOfMemoryError
-XX:HeapDumpPath=/var/log/dremio
-XX:InitialHeapSize=1073741824
-XX:InitiatingHeapOccupancyPercent=25
-XX:MaxDirectMemorySize=64424509440
-XX:MaxHeapSize=64424509440
-XX:MaxNewSize=20971520000
-XX:MinHeapDeltaBytes=524288
-XX:NewSize=703594496
-XX:NumberOfGCLogFiles=5
-XX:OldSize=1408237568
-XX:+PrintClassHistogramAfterFullGC
-XX:+PrintClassHistogramBeforeFullGC
-XX:+PrintGC
-XX:+PrintGCTimeStamps
-XX:+UseFastUnorderedTimeStamps
-XX:+UseGCLogFileRotation
-XX:+UseParallelGC";
        assert_eq!(flags.to_string(), expected.to_string());
    }

    #[test]
    fn test_parse_collector() {
        let g1_gc = get_collector(&vec!["-XX:+UseG1GC".to_string(), "".to_string()]);
        assert_eq!(g1_gc, Collector::G1GC);
        let parallel_gc = get_collector(&vec!["-XX:+UseParallelGC".to_string(), "".to_string()]);
        assert_eq!(parallel_gc, Collector::Parallel);
        let cms = get_collector(&vec!["-XX:+UseParNewGC".to_string(), "".to_string()]);
        assert_eq!(cms, Collector::CMS);
        let serial = get_collector(&vec!["-XX:+UseSerialGC".to_string(), "".to_string()]);
        assert_eq!(serial, Collector::SerialGC);
        let zgc = get_collector(&vec!["-XX:+UseZGC".to_string(), "".to_string()]);
        assert_eq!(zgc, Collector::ZGC);
        let shenandoah = get_collector(&vec!["-XX:+UseShenandoahGC".to_string(), "".to_string()]);
        assert_eq!(shenandoah, Collector::Shenandoah);
        let unknown = get_collector(&vec!["-XX:+UseBobGC".to_string(), "".to_string()]);
        assert_eq!(unknown, Collector::Unknown);
    }

    #[test]
    fn test_parse_gc_flags() {
        let line = "CommandLine flags: -XX:+DisableExplicitGC -XX:ErrorFile=/opt/dremio/data/hs_err_pid%p.log -XX:G1HeapRegionSize=33554432 -XX:GCLogFileSize=4096000 -XX:+HeapDumpOnOutOfMemoryError -XX:HeapDumpPath=/opt/dremio/data/ -XX:InitialHeapSize=2048000000 -XX:InitiatingHeapOccupancyPercent=25 -XX:MaxDirectMemorySize=120259084288 -XX:MaxGCPauseMillis=500 -XX:MaxHeapSize=17179869184 -XX:NumberOfGCLogFiles=5 -XX:+PrintClassHistogramAfterFullGC -XX:+PrintClassHistogramBeforeFullGC -XX:+PrintGC -XX:+PrintGCDateStamps -XX:+PrintGCDetails -XX:+PrintGCTimeStamps -XX:+UseCompressedClassPointers -XX:+UseCompressedOops -XX:+UseG1GC -XX:+UseGCLogFileRotation";
        let gc_flags = parse_gc_flags(line.to_string(), 128 * 1000 * 1000 * 1000);
        assert_eq!(gc_flags.collector, Collector::G1GC);
        assert_approx_equal(gc_flags.region_size_mb, 32.0, 0.01);
        assert_approx_equal(gc_flags.max_heap_size_gb, 16.0, 0.01);
        assert_approx_equal(gc_flags.min_heap_size_gb, 1.907, 0.01);
        assert_eq!(gc_flags.target_pause_millis, 500);
    }
}
