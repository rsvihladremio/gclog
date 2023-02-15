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

use std::{
    error::Error,
    fs::File,
    io::{BufRead, BufReader},
};

use super::{
    flags::{parse_gc_flags, Collector, GCFlags},
    jvmstats::{parse_jdk_stats, parse_memory, MemoryStats},
    pauses::{
        generate_pause_table, has_gc, parse_full_gc_pause, parse_gc_pause, show_max_pause_times,
        GCPause,
    },
    recommendations::generate_recommendations,
};

fn is_log_finished(multiline_log: &str) -> bool {
    let mut start = 0;
    let mut end = 0;
    let mut skip = false;
    for line in multiline_log.split('\n') {
        if line.trim().starts_with("Total") {
            skip = false;
        }
        if skip {
            continue;
        }
        //special case for evil histograms that add bogus [ to my [] driven log format
        if line.contains("Histogram") {
            skip = true;
        }
        let open_count = line.matches('[').count();
        start += open_count;
        let close_count = line.matches(']').count();
        end += close_count;
    }
    start == end
}
pub fn exec(file_name: String) -> Result<String, Box<dyn Error>> {
    let file = File::open(file_name)?;
    let reader = BufReader::new(file);

    let mut pauses: Vec<GCPause> = vec![];
    let mut gc_flags: GCFlags = GCFlags {
        collector: Collector::Unknown,
        max_heap_size_gb: 0.0,
        min_heap_size_gb: 0.0,
        region_size_mb: 0.0,
        target_pause_millis: 0,
        max_direct_memory_gb: 0.0,
        all_flags: vec![],
    };
    let mut jdk_stats: String = "".to_string();
    let mut memory_stats: MemoryStats = MemoryStats {
        physical_memory_str: "".to_string(),
        physical_memory_bytes: 0,
    };
    let mut read_multiline = false;
    let mut multiline_log: String = "".to_string();
    let open_bracket = '[';
    let close_bracket = ']';
    let mut already_parsed_cmd = false;
    let mut already_parsed_flags = false;
    let mut already_parsed_memory = false;
    for line_result in reader.lines() {
        let line = line_result?;

        if !already_parsed_cmd && line.starts_with("CommandLine flags: ") {
            already_parsed_cmd = true;
            gc_flags = parse_gc_flags(line, memory_stats.physical_memory_bytes);
        } else if !already_parsed_memory && line.starts_with("Memory: ") {
            already_parsed_memory = true;
            memory_stats = parse_memory(line);
        } else if !already_parsed_flags
            && (line.starts_with("OpenJDK ") || line.starts_with("Java"))
        {
            already_parsed_flags = true;
            jdk_stats = parse_jdk_stats(line)
        } else if read_multiline {
            let new_line = format!("\n{line}");
            let new_line_str = new_line.as_str();
            multiline_log += new_line_str;
            if line.contains(']') {
                //first check if log is finished really or not
                if !is_log_finished(&multiline_log) {
                    continue;
                }
                read_multiline = false;
                let pause = parse_full_gc_pause(multiline_log)?;
                multiline_log = "".to_string();
                if pause.gc_type != "after full gc" {
                    pauses.push(pause);
                }
            }
        } else if !read_multiline
            && line.chars().filter(|x| x == &open_bracket).count()
                != line.chars().filter(|x| x == &close_bracket).count()
        {
            read_multiline = true;
            let new_line = format!("{line}\n");
            let new_line_str = new_line.as_str();
            multiline_log.push_str(new_line_str);
        } else if has_gc(&line) {
            let pause: GCPause = parse_gc_pause(line)?;
            pauses.push(pause);
        }
    }
    let headline_max = "Max Pause:".to_string();
    let underline_max = "--------".to_string();
    let pause_table_max = show_max_pause_times(&pauses);

    let headline = "GC Summary:".to_string();
    let underline = "--------".to_string();
    let pause_table = generate_pause_table(&pauses);

    let recommendations = generate_recommendations(&gc_flags, &pauses);
    Ok([
        headline,
        underline,
        jdk_stats,
        memory_stats.physical_memory_str,
        gc_flags.to_string(),
        headline_max,
        underline_max,
        pause_table_max,
        pause_table,
        recommendations,
    ]
    .join("\n"))
}

#[cfg(test)]
mod tests {
    use crate::glog::exec::exec;
    use std::io::Write;
    use tempfile::NamedTempFile;

    use super::is_log_finished;

    #[test]
    fn test_is_log_finished() {
        let log_with_histo = "2022-01-02T11:11:01.111+0000: 234567.120: [Full GC (Allocation Failure) 2022-01-02T11:11:01.111+0000: 234567.120: [Class Histogram (before full gc):
num     #instances         #bytes  class name
----------------------------------------------
    1:       9013468      888888888  org.apache.arrow.memory.ArrowBuf
    20:        431460       91111101  [B
    10821:             1             16  sun.util.resources.LocaleData$LocaleDataResourceBundleControl
Total      99999999     8643256886
, 1.1111111 secs]
";
        let result = is_log_finished(log_with_histo);
        assert!(!result);
        let log_with_histo_and_gc_time = "2022-01-02T11:11:01.111+0000: 234567.120: [Full GC (Allocation Failure) 2022-01-02T11:11:01.111+0000: 234567.120: [Class Histogram (before full gc):
num     #instances         #bytes  class name
----------------------------------------------
    20:        431460       91111101  [B
Total      99999999     8643256886
, 1.1111111 secs]
    4185M->1198M(5336M), 4.3449655 secs]
";
        let finished_result = is_log_finished(log_with_histo_and_gc_time);
        assert!(finished_result);
    }

    #[test]
    fn test_full_gc_parse_with_softreferences() {
        // Create a file inside of `std::env::temp_dir()`.
        let mut file = NamedTempFile::new().expect("unable to make tmp file");
        let jdk_stats = "OpenJDK 64-Bit Server VM (25.332-b09) for linux-amd64 JRE (1.8.0_332-b09), built on Apr 20 2022 08:18:57 by \"openjdk\" with gcc 4.4.7 20120313 (Red Hat 4.4.7-23)";
        writeln!(file, "{jdk_stats}").unwrap();
        let memory_stats =
            "Memory: 4k page, physical 128000000k(127996468k free), swap 0k(0k free)";
        writeln!(file, "{memory_stats}").unwrap();
        let gc_flags = "CommandLine flags: -XX:+DisableExplicitGC -XX:ErrorFile=/opt/dremio/data/hs_err_pid%p.log -XX:G1HeapRegionSize=33554432 -XX:GCLogFileSize=4096000 -XX:+HeapDumpOnOutOfMemoryError -XX:HeapDumpPath=/opt/dremio/data/ -XX:InitialHeapSize=2048000000 -XX:InitiatingHeapOccupancyPercent=25 -XX:MaxDirectMemorySize=120259084288 -XX:MaxGCPauseMillis=500 -XX:MaxHeapSize=17179869184 -XX:NumberOfGCLogFiles=5 -XX:+PrintClassHistogramAfterFullGC -XX:+PrintClassHistogramBeforeFullGC -XX:+PrintGC -XX:+PrintGCDateStamps -XX:+PrintGCDetails -XX:+PrintGCTimeStamps -XX:+UseCompressedClassPointers -XX:+UseCompressedOops -XX:+UseG1GC -XX:+UseGCLogFileRotation";
        writeln!(file, "{gc_flags}").unwrap();
        let humongous_gc_log = "2021-02-22T01:01:02.120+0000: 22000.498: [GC pause (G1 Humongous Allocation) (young) (initial-mark), 0.0911111 secs]";
        writeln!(file, "{humongous_gc_log}").unwrap();
        let full_gc_line = "2022-08-24T01:54:38.603+0000: 190268.356: [Full GC (Allocation Failure) 2022-08-24T01:54:38.603+0000: 190268.356: [Class Histogram (before full gc): 
num     #instances         #bytes  class name
----------------------------------------------
    1:        954593     4097572584  [B
    2:        135687      723913256  [I
    3:       1676254      265188008  [J
    4:       2769733      177262912  org.apache.arrow.memory.ArrowBuf
    5:       1774936      156194368  io.netty.buffer.PooledUnsafeDirectByteBuf
    6:       1753854      126277488  org.apache.arrow.memory.NettyAllocationManager
    7:       2080367       99729672  [Ljava.lang.Object;
    8:       1753894       98218064  org.apache.arrow.memory.BufferLedger
    9:       1476241       94479424  io.netty.buffer.PoolSubpage
Total      34967203     6624451024
, 0.9324195 secs]
2022-08-24T01:54:40.318+0000: 190270.071: [SoftReference, 24521 refs, 0.0035689 secs]2022-08-24T01:54:40.322+0000: 190270.074: [WeakReference, 24515 refs, 0.0017063 secs]2022-08-24T01:54:40.323+0000: 190270.076: [FinalReference, 1360 refs, 0.0003365 secs]2022-08-24T01:54:40.324+0000: 190270.076: [PhantomReference, 0 refs, 12778 refs, 0.0004892 secs]2022-08-24T01:54:40.324+0000: 190270.077: [JNI Weak Reference, 0.0000910 secs] 16364M->4966M(16384M), 3.6564555 secs]
    [Eden: 0.0B(800.0M)->0.0B(8064.0M) Survivors: 0.0B->0.0B Heap: 16364.4M(16384.0M)->4966.6M(16384.0M)], [Metaspace: 168826K->155496K(1314816K)]
    2022-08-24T01:54:42.260+0000: 190272.012: [Class Histogram (after full gc): 
    num     #instances         #bytes  class name
    ----------------------------------------------
    1:        891298     4042599072  [B
    2:        909679      182473568  [J
    3:       1457412       93274368  org.apache.arrow.memory.ArrowBuf
    4:        980472       86281536  io.netty.buffer.PooledUnsafeDirectByteBuf
    5:       1131756       71520808  [Ljava.lang.Object;
    Total      21384721     5207859016
    , 0.4783018 secs]
    [Times: user=4.76 sys=0.97, real=4.14 secs]
";
        writeln!(file, "{full_gc_line}").unwrap();
        let new_file = file.into_temp_path();
        let new_file_str = new_file.to_str().expect("cannot read file");
        let parsed = exec(new_file_str.to_string()).expect("failed to parse");
        assert!(
            parsed.contains("Full GC - (Allocation Failure)"),
            "was {parsed}"
        );
        assert!(
            parsed.contains("3.66"),
            "expected 3.66 in the output but had {parsed}"
        );
    }

    #[test]
    fn test_full_gc_parse() {
        // Create a file inside of `std::env::temp_dir()`.
        let mut file = NamedTempFile::new().expect("unable to make tmp file");
        let jdk_stats = "OpenJDK 64-Bit Server VM (25.332-b09) for linux-amd64 JRE (1.8.0_332-b09), built on Apr 20 2022 08:18:57 by \"openjdk\" with gcc 4.4.7 20120313 (Red Hat 4.4.7-23)";
        writeln!(file, "{jdk_stats}").unwrap();
        let memory_stats =
            "Memory: 4k page, physical 128000000k(127996468k free), swap 0k(0k free)";
        writeln!(file, "{memory_stats}").unwrap();
        let gc_flags = "CommandLine flags: -XX:+DisableExplicitGC -XX:ErrorFile=/opt/dremio/data/hs_err_pid%p.log -XX:G1HeapRegionSize=33554432 -XX:GCLogFileSize=4096000 -XX:+HeapDumpOnOutOfMemoryError -XX:HeapDumpPath=/opt/dremio/data/ -XX:InitialHeapSize=2048000000 -XX:InitiatingHeapOccupancyPercent=25 -XX:MaxDirectMemorySize=120259084288 -XX:MaxGCPauseMillis=500 -XX:MaxHeapSize=17179869184 -XX:NumberOfGCLogFiles=5 -XX:+PrintClassHistogramAfterFullGC -XX:+PrintClassHistogramBeforeFullGC -XX:+PrintGC -XX:+PrintGCDateStamps -XX:+PrintGCDetails -XX:+PrintGCTimeStamps -XX:+UseCompressedClassPointers -XX:+UseCompressedOops -XX:+UseG1GC -XX:+UseGCLogFileRotation";
        writeln!(file, "{gc_flags}").unwrap();
        let humongous_gc_log = "2021-02-22T01:01:02.120+0000: 22000.498: [GC pause (G1 Humongous Allocation) (young) (initial-mark), 0.0911111 secs]";
        writeln!(file, "{humongous_gc_log}").unwrap();
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
        writeln!(file, "{full_gc_line}").unwrap();
        let new_file = file.into_temp_path();
        let new_file_str = new_file.to_str().expect("cannot read file");
        let parsed = exec(new_file_str.to_string()).expect("failed to parse");
        assert!(
            parsed.contains("Full GC - (Allocation Failure)"),
            "was {parsed}"
        );
        assert!(
            parsed.contains("4.34"),
            "expected 4.34 in the output but had {parsed}"
        );
    }

    #[test]
    fn test_without_headers() {
        let mut file = NamedTempFile::new().expect("unable to make tmp file");
        let lines = "

2022-01-02T11:11:01.111+0000: 6993.481: [GC pause (G1 Evacuation Pause) (young), 0.0709341 secs]
    [Parallel Time: 60.5 ms, GC Workers: 23]
        [GC Worker Start (ms): Min: 6993481.9, Avg: 6993482.5, Max: 6993483.1, Diff: 1.2]
        [Ext Root Scanning (ms): Min: 1.0, Avg: 4.2, Max: 58.9, Diff: 57.9, Sum: 97.5]
        [Update RS (ms): Min: 0.0, Avg: 36.6, Max: 39.7, Diff: 39.7, Sum: 842.6]
            [Processed Buffers: Min: 0, Avg: 52.3, Max: 67, Diff: 67, Sum: 1203]
        [Scan RS (ms): Min: 0.0, Avg: 0.7, Max: 0.8, Diff: 0.8, Sum: 15.2]
        [Code Root Scanning (ms): Min: 0.0, Avg: 0.0, Max: 0.0, Diff: 0.0, Sum: 0.1]
        [Object Copy (ms): Min: 0.5, Avg: 17.4, Max: 18.3, Diff: 17.8, Sum: 399.7]
        [Termination (ms): Min: 0.0, Avg: 0.0, Max: 0.0, Diff: 0.0, Sum: 0.3]
            [Termination Attempts: Min: 1, Avg: 1.1, Max: 2, Diff: 1, Sum: 25]
        [GC Worker Other (ms): Min: 0.0, Avg: 0.2, Max: 0.4, Diff: 0.4, Sum: 4.8]
        [GC Worker Total (ms): Min: 58.4, Avg: 59.1, Max: 59.8, Diff: 1.4, Sum: 1360.3]
        [GC Worker End (ms): Min: 6993541.4, Avg: 6993541.6, Max: 6993541.8, Diff: 0.4]
    [Code Root Fixup: 0.0 ms]
    [Code Root Purge: 0.0 ms]
    [Clear CT: 1.4 ms]
    [Other: 9.0 ms]
        [Choose CSet: 0.0 ms]
        [Ref Proc: 3.4 ms]
        [Ref Enq: 0.2 ms]
        [Redirty Cards: 0.7 ms]
        [Humongous Register: 0.3 ms]
        [Humongous Reclaim: 2.4 ms]
        [Free CSet: 1.5 ms]
    [Eden: 10.9G(10.9G)->0.0B(10.9G) Survivors: 192.0M->224.0M Heap: 14.5G(18.5G)->1830.3M(18.5G)]
    [Times: user=1.17 sys=0.23, real=0.07 secs]
2022-01-02T11:11:01.111+0000: 7004.927: [GC pause (G1 Evacuation Pause) (young), 0.2189828 secs]
    [Parallel Time: 205.6 ms, GC Workers: 23]
        [GC Worker Start (ms): Min: 7004928.5, Avg: 7004929.4, Max: 7004930.3, Diff: 1.7]
        [Ext Root Scanning (ms): Min: 1.9, Avg: 11.8, Max: 204.7, Diff: 202.8, Sum: 272.1]
        [Update RS (ms): Min: 0.0, Avg: 4.2, Max: 6.2, Diff: 6.2, Sum: 95.8]
            [Processed Buffers: Min: 0, Avg: 6.6, Max: 23, Diff: 23, Sum: 151]
        [Scan RS (ms): Min: 0.0, Avg: 0.1, Max: 0.3, Diff: 0.2, Sum: 2.3]
        [Code Root Scanning (ms): Min: 0.0, Avg: 0.0, Max: 0.1, Diff: 0.1, Sum: 0.5]
        [Object Copy (ms): Min: 0.0, Avg: 28.2, Max: 30.1, Diff: 30.1, Sum: 647.8]
        [Termination (ms): Min: 0.0, Avg: 159.7, Max: 167.3, Diff: 167.3, Sum: 3673.4]
            [Termination Attempts: Min: 1, Avg: 267.4, Max: 321, Diff: 320, Sum: 6151]
        [GC Worker Other (ms): Min: 0.0, Avg: 0.1, Max: 0.3, Diff: 0.3, Sum: 2.8]
        [GC Worker Total (ms): Min: 203.2, Avg: 204.1, Max: 205.2, Diff: 1.9, Sum: 4694.6]
        [GC Worker End (ms): Min: 7005133.4, Avg: 7005133.5, Max: 7005133.8, Diff: 0.5]
    [Code Root Fixup: 0.0 ms]
    [Code Root Purge: 0.0 ms]
    [Clear CT: 4.3 ms]
    [Other: 9.0 ms]
";

        write!(file, "{lines}").unwrap();
        let new_file = file.into_temp_path();
        let new_file_str = new_file.to_str().unwrap();
        let parsed = exec(new_file_str.to_string()).expect("failed to parse");
        assert!(
            parsed.contains("G1 Evacuation Pause"),
            "did not find G1 Evacuation Pause file has {parsed}"
        );
        assert!(
            parsed.contains("0.07"),
            "did not find 0.07. File has {parsed}"
        );
    }

    #[test]
    fn test_with_ergonomics() {
        let mut file = NamedTempFile::new().expect("unable to make tmp file");

        let lines = "2022-07-22 18:41:06 GC log file created /opt/dremio/data/gc.log.4
OpenJDK 64-Bit Server VM (25.332-b09) for linux-amd64 JRE (1.8.0_332-b09), built on Apr 20 2022 08:18:57 by \"openjdk\" with gcc 4.4.7 20120313 (Red Hat 4.4.7-23)
Memory: 4k page, physical 122412460k(86529060k free), swap 0k(0k free)
CommandLine flags: -XX:CICompilerCount=4 -XX:ConcGCThreads=2 -XX:ErrorFile=/opt/dremio/data/hs_err_pid%p.log -XX:G1HeapRegionSize=33554432 -XX:GCLogFileSize=4096000 -XX:+HeapDumpOnOutOfMemoryError -XX:HeapDumpPath=/opt/dremio/data -XX:InitialHeapSize=1979711488 -XX:InitiatingHeapOccupancyPercent=25 -XX:MarkStackSize=4194304 -XX:MaxDirectMemorySize=2147483648 -XX:MaxGCPauseMillis=500 -XX:MaxHeapSize=17179869184 -XX:MaxNewSize=10301210624 -XX:MinHeapDeltaBytes=33554432 -XX:NumberOfGCLogFiles=10 -XX:+PrintAdaptiveSizePolicy -XX:+PrintClassHistogramAfterFullGC -XX:+PrintClassHistogramBeforeFullGC -XX:+PrintGC -XX:+PrintGCDateStamps -XX:+PrintGCDetails -XX:+PrintGCTimeStamps -XX:+PrintReferenceGC -XX:+UseCompressedClassPointers -XX:+UseCompressedOops -XX:+UseFastUnorderedTimeStamps -XX:+UseG1GC -XX:+UseGCLogFileRotation 
2022-07-22T18:41:06.240+0000: 54055.679: [GC pause (G1 Evacuation Pause) (young) 54055.679: [G1Ergonomics (CSet Construction) start choosing CSet, _pending_cards: 3785, predicted base time: 8.89 ms, remaining time: 491.11 ms, target pause time: 500.00 ms]
54055.679: [G1Ergonomics (CSet Construction) add young regions to CSet, eden: 62 regions, survivors: 0 regions, predicted young region time: 298.30 ms]
54055.679: [G1Ergonomics (CSet Construction) finish choosing CSet, eden: 62 regions, survivors: 0 regions, old: 0 regions, predicted pause time: 307.19 ms, target pause time: 500.00 ms]
2022-07-22T18:41:06.509+0000: 54055.947: [SoftReference, 0 refs, 0.0000831 secs]2022-07-22T18:41:06.509+0000: 54055.947: [WeakReference, 343 refs, 0.0000987 secs]2022-07-22T18:41:06.509+0000: 54055.947: [FinalReference, 3 refs, 0.0000209 secs]2022-07-22T18:41:06.509+0000: 54055.947: [PhantomReference, 5 refs, 2 refs, 0.0000114 secs]2022-07-22T18:41:06.509+0000: 54055.947: [JNI Weak Reference, 0.0000921 secs] 54055.952: [G1Ergonomics (Heap Sizing) attempt heap expansion, reason: recent GC overhead higher than threshold after GC, recent GC overhead: 44.66 %, threshold: 10.00 %, uncommitted: 5301600256 bytes, calculated expansion amount: 1060320051 bytes (20.00 %)]
54055.952: [G1Ergonomics (Heap Sizing) expand the heap, requested expansion amount: 1060320051 bytes, attempted expansion amount: 1073741824 bytes]
54055.954: [G1Ergonomics (Concurrent Cycles) request concurrent cycle initiation, reason: occupancy higher than threshold, occupancy: 4664066048 bytes, allocation request: 0 bytes, threshold: 3238002675 bytes (25.00 %), source: end of GC]
, 0.2753957 secs]";
        write!(file, "{lines}").unwrap();
        let new_file = file.into_temp_path();
        let new_file_str = new_file.to_str().unwrap();
        let parsed = exec(new_file_str.to_string()).expect("failed to parse");
        assert!(
            parsed.contains("G1 Evacuation Pause"),
            "did not find G1 Evacuation Pause file has {parsed}"
        );
        assert!(
            parsed.contains("0.28"),
            "did not find 0.28. File has {parsed}"
        );
    }

    #[test]
    fn test_full_gc_parse_single_line_with_extra_line() {
        // Create a file inside of `std::env::temp_dir()`.
        let mut file = NamedTempFile::new().expect("unable to make tmp file");
        writeln!(
            file,
            "not great stuff to be starting a file withm see if dqdrust ignores me"
        )
        .unwrap();
        let jdk_stats = "OpenJDK 64-Bit Server VM (25.332-b09) for linux-amd64 JRE (1.8.0_332-b09), built on Apr 20 2022 08:18:57 by \"openjdk\" with gcc 4.4.7 20120313 (Red Hat 4.4.7-23)";
        writeln!(file, "{jdk_stats}").unwrap();
        let memory_stats =
            "Memory: 4k page, physical 128000000k(127996468k free), swap 0k(0k free)";
        writeln!(file, "{memory_stats}").unwrap();
        let gc_flags = "CommandLine flags: -XX:+DisableExplicitGC -XX:ErrorFile=/opt/dremio/data/hs_err_pid%p.log -XX:G1HeapRegionSize=33554432 -XX:GCLogFileSize=4096000 -XX:+HeapDumpOnOutOfMemoryError -XX:HeapDumpPath=/opt/dremio/data/ -XX:InitialHeapSize=2048000000 -XX:InitiatingHeapOccupancyPercent=25 -XX:MaxDirectMemorySize=120259084288 -XX:MaxGCPauseMillis=500 -XX:MaxHeapSize=17179869184 -XX:NumberOfGCLogFiles=5 -XX:+PrintClassHistogramAfterFullGC -XX:+PrintClassHistogramBeforeFullGC -XX:+PrintGC -XX:+PrintGCDateStamps -XX:+PrintGCDetails -XX:+PrintGCTimeStamps -XX:+UseCompressedClassPointers -XX:+UseCompressedOops -XX:+UseG1GC -XX:+UseGCLogFileRotation";
        writeln!(file, "{gc_flags}").unwrap();
        let humongous_gc_log = "2021-02-22T01:01:02.120+0000: 22000.498: [GC pause (G1 Humongous Allocation) (young) (initial-mark), 0.0911111 secs]";
        writeln!(file, "{humongous_gc_log}").unwrap();
        let new_file = file.into_temp_path();
        let new_file_str = new_file.to_str().unwrap();
        let parsed = exec(new_file_str.to_string()).expect("failed to parse");
        assert!(
            parsed.contains("G1 Humongous Allocation"),
            "did not find humgous allocation file has {parsed}"
        );
        assert!(
            parsed.contains("0.09"),
            "did not find 0.09. File has {parsed}"
        );
    }
}
