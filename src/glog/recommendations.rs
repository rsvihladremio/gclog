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

use super::{
    flags::{Collector, GCFlags},
    pauses::{GCPause, HeapSizing},
};

pub fn generate_recommendations(flags: &GCFlags, pauses: &Vec<GCPause>) -> String {
    let mut recs: Vec<String> = vec![];
    let mut to_space_exhausted = 0;
    let mut to_space_exhausted_total_pause_time = 0.0;
    let mut to_space_exhausted_max_pause_time: f64 = 0.0;
    let total_pauses = pauses.len();
    let mut total_full_gcs = 0;
    let mut full_gc_max_pause_time: f64 = 0.0;
    let mut full_gc_total_pause_time: f64 = 0.0;
    let mut humongous_collections = 0;
    let mut humongous_collections_total_pause_time = 0.0;
    let mut total_resizes_up = 0;
    let mut total_resizes_down = 0;
    for pause in pauses {
        if pause.heap_sizing == HeapSizing::Expansion {
            total_resizes_up += 1;
        } else if pause.heap_sizing == HeapSizing::Shrinking {
            total_resizes_down += 1;
        }

        if pause.attributes.contains(&"to-space exhausted".to_string()) {
            to_space_exhausted += 1;
            to_space_exhausted_total_pause_time += pause.pause_time_seconds;
            to_space_exhausted_max_pause_time =
                to_space_exhausted_max_pause_time.max(pause.pause_time_seconds);
        }
        if pause.gc_type == "G1 Humongous Allocation" {
            humongous_collections += 1;
            humongous_collections_total_pause_time += pause.pause_time_seconds;
        } else if pause.is_full_gc {
            total_full_gcs += 1;
            full_gc_max_pause_time = full_gc_max_pause_time.max(pause.pause_time_seconds);
            full_gc_total_pause_time += pause.pause_time_seconds;
        }
    }
    match flags.collector {
        Collector::SerialGC =>
            recs.push("* Serial GC collector detected. This is an older collector and is only intended for single core machines. Use G1GC instead.".to_string()),
        Collector::G1GC =>{
            if to_space_exhausted > 0 {
                let per_total_pauses = (to_space_exhausted as f64 / total_pauses as f64) * 100.0;
                recs.push(format!("* {per_total_pauses:.2}% of GCs were to-space exhausted adding {to_space_exhausted_total_pause_time:.2} total seconds pause time with a max pause time of {to_space_exhausted_max_pause_time:.2} seconds, this means the max heap size was too small for use case during that time. Raising the heap size will help minimize the chance of this occuring again."));
            }
            if humongous_collections > 0 {
                let per_total_pauses = (humongous_collections as f64 / total_pauses as f64) * 100.0;
                let diff = (flags.region_size_mb - 32.0).abs();
                let recommend_new_region_size = if diff < 0.02 {
                    "Region size is already maxed out at 32.0 mb. Therefore one either needs to change the gc collector from G1GC or begin looking for expensive queries or system bugs".to_string()
                } else {
                    format!(
                        "Region size is {:.1} mb. Consider raising it up to {:.1} mb",
                        flags.region_size_mb,
                        (flags.region_size_mb as u32 + 1_u32).next_power_of_two() as f32,
                    )
                };
                recs.push(format!(
                    "* {per_total_pauses:.2}% of GCs were humongous allocations adding {humongous_collections_total_pause_time:.2} total seconds pause time, this indicates there are objects to big for your GC configuration. {recommend_new_region_size}"
                ))
            }
        },
        Collector::CMS =>
        recs.push("* CMS GC collector detected. This is an older collector and is removed in java 14. This can actually be a very performant collector, and if the machine is well tuned, it is best to leave it as it was. However, if you intend to raise the heap size consider the G1GC collector.".to_string()),
        Collector::Parallel =>
        recs.push("* Parallel GC collector detected. This is an older collector and it will lead to long pauses. Use G1GC instead.".to_string()),
        Collector::ZGC =>
        recs.push("* ZGC GC collector detected. This is a newer collector optimized for shorter gc pauses, however, it has some known issues with dremio (see https://dremio.atlassian.net/browse/DX-46569?focusedCommentId=494918). Consider using G1GC or CMS instead.".to_string()),
        Collector::Shenandoah =>
        recs.push("* Shenandoah GC collector detected. This is a newer collector and is not yet full supported by Dremio (see https://dremio.atlassian.net/browse/DX-37567) and there may be some unexpected behavior consider using the G1GC collector.".to_string()),
        Collector::Unknown =>
        recs.push("* Unknown GC collector detected. Review the JVM flags and try and submit a bug report for this new collector to https://dremio.atlassian.net/jira/software/c/projects/ST/issues/?filter=allissues".to_string()),
    }

    if total_full_gcs > 0 && Collector::Parallel != flags.collector {
        recs.push(format!("* {:.2}% of GCs were Full GCs adding {:.2} total seconds pause time with a max pause time of {:.2} seconds,  this means the max heap size was too small for use case during that time. Raising the heap size will help minimize the chance of this occuring.", total_full_gcs as f64/ total_pauses as f64, full_gc_total_pause_time ,full_gc_max_pause_time));
    }

    let total_resize_attempts = total_resizes_up + total_resizes_down;
    if total_resize_attempts > 0 && (flags.max_heap_size_gb - flags.min_heap_size_gb).abs() > 0.01 {
        recs.push(format!(
            "* {:.2}% of GCs attempted to resize the JVM {} pauses sized up the GC, {} pauses sized down the GC. This adds additional time to the GC and makes pause times more variable, to resolve this set Xms and Xms to be the same, read https://blog.gceasy.io/2017/08/15/gc-log-standardization-api/ and https://docs.oracle.com/javase/9/gctuning/garbage-first-garbage-collector-tuning.htm#JSGCT-GUID-90E30ACA-8040-432E-B3A0-1E0440AB556A",
            total_resize_attempts as f64 / total_pauses as f64,
            total_resizes_up,
            total_resizes_down,
        ))
    }

    if recs.is_empty() {
        return "".to_string();
    }
    [
        "recommendations".to_string(),
        "---------------".to_string(),
        recs.join("\n"),
    ]
    .join("\n")
}

#[cfg(test)]
mod tests {
    use crate::glog::{
        flags::GCFlags,
        pauses::{GCPause, HeapSizing},
        recommendations::generate_recommendations,
    };

    #[test]
    fn test_g1gc_recommendations_humongous_allocations_with_region_below() {
        let pauses = vec![
            GCPause {
                attributes: vec!["young".to_string()],
                gc_type: "G1 Humongous Allocation".to_string(),
                is_full_gc: false,
                pause_time_seconds: 3.10,
                time_epoch: 1658739348,
                heap_sizing: HeapSizing::None,
            },
            GCPause {
                attributes: vec!["young".to_string()],
                gc_type: "G1 Humongous Allocation".to_string(),
                is_full_gc: false,
                pause_time_seconds: 3.10,
                time_epoch: 1658739348,
                heap_sizing: HeapSizing::None,
            },
        ];
        let recs = generate_recommendations(
            &GCFlags {
                collector: crate::glog::flags::Collector::G1GC,
                max_heap_size_gb: 32.0,
                min_heap_size_gb: 32.0,
                region_size_mb: 1.0,
                target_pause_millis: 500,
                max_direct_memory_gb: 40.0,
                all_flags: vec![],
            },
            &pauses,
        );
        assert!(!recs.is_empty());
        assert_eq!(recs, "recommendations
---------------
* 100.00% of GCs were humongous allocations adding 6.20 total seconds pause time, this indicates there are objects to big for your GC configuration. Region size is 1.0 mb. Consider raising it up to 2.0 mb");

        let recs = generate_recommendations(
            &GCFlags {
                collector: crate::glog::flags::Collector::G1GC,
                max_heap_size_gb: 32.0,
                min_heap_size_gb: 32.0,
                region_size_mb: 2.0,
                target_pause_millis: 500,
                max_direct_memory_gb: 40.0,
                all_flags: vec![],
            },
            &pauses,
        );
        assert!(!recs.is_empty());
        assert_eq!(recs, "recommendations
---------------
* 100.00% of GCs were humongous allocations adding 6.20 total seconds pause time, this indicates there are objects to big for your GC configuration. Region size is 2.0 mb. Consider raising it up to 4.0 mb");
        let recs = generate_recommendations(
            &GCFlags {
                collector: crate::glog::flags::Collector::G1GC,
                max_heap_size_gb: 32.0,
                min_heap_size_gb: 32.0,
                region_size_mb: 4.0,
                target_pause_millis: 500,
                max_direct_memory_gb: 40.0,
                all_flags: vec![],
            },
            &pauses,
        );
        assert!(!recs.is_empty());
        assert_eq!(recs, "recommendations
---------------
* 100.00% of GCs were humongous allocations adding 6.20 total seconds pause time, this indicates there are objects to big for your GC configuration. Region size is 4.0 mb. Consider raising it up to 8.0 mb");
        let recs = generate_recommendations(
            &GCFlags {
                collector: crate::glog::flags::Collector::G1GC,
                max_heap_size_gb: 32.0,
                min_heap_size_gb: 32.0,
                region_size_mb: 8.0,
                target_pause_millis: 500,
                max_direct_memory_gb: 40.0,
                all_flags: vec![],
            },
            &pauses,
        );
        assert!(!recs.is_empty());
        assert_eq!(recs, "recommendations
---------------
* 100.00% of GCs were humongous allocations adding 6.20 total seconds pause time, this indicates there are objects to big for your GC configuration. Region size is 8.0 mb. Consider raising it up to 16.0 mb");
        let recs = generate_recommendations(
            &GCFlags {
                collector: crate::glog::flags::Collector::G1GC,
                max_heap_size_gb: 32.0,
                min_heap_size_gb: 32.0,
                region_size_mb: 16.0,
                target_pause_millis: 500,
                max_direct_memory_gb: 40.0,
                all_flags: vec![],
            },
            &pauses,
        );
        assert!(!recs.is_empty());
        assert_eq!(recs, "recommendations
---------------
* 100.00% of GCs were humongous allocations adding 6.20 total seconds pause time, this indicates there are objects to big for your GC configuration. Region size is 16.0 mb. Consider raising it up to 32.0 mb");
    }

    #[test]
    fn test_g1gc_recommendations_humongous_allocations_with_region_at_max() {
        let pauses = vec![
            GCPause {
                attributes: vec!["young".to_string()],
                gc_type: "G1 Humongous Allocation".to_string(),
                is_full_gc: false,
                pause_time_seconds: 3.10,
                time_epoch: 1658739348,
                heap_sizing: HeapSizing::None,
            },
            GCPause {
                attributes: vec!["young".to_string()],
                gc_type: "G1 Humongous Allocation".to_string(),
                is_full_gc: false,
                pause_time_seconds: 3.10,
                time_epoch: 1658739348,
                heap_sizing: HeapSizing::None,
            },
        ];
        let recs = generate_recommendations(
            &GCFlags {
                collector: crate::glog::flags::Collector::G1GC,
                max_heap_size_gb: 32.0,
                min_heap_size_gb: 32.0,
                region_size_mb: 32.0,
                target_pause_millis: 500,
                max_direct_memory_gb: 40.0,
                all_flags: vec![],
            },
            &pauses,
        );
        assert!(!recs.is_empty());
        assert_eq!(recs, "recommendations
---------------
* 100.00% of GCs were humongous allocations adding 6.20 total seconds pause time, this indicates there are objects to big for your GC configuration. Region size is already maxed out at 32.0 mb. Therefore one either needs to change the gc collector from G1GC or begin looking for expensive queries or system bugs");
    }

    #[test]
    fn test_recommendations_are_empty() {
        let recs = generate_recommendations(
            &GCFlags {
                collector: crate::glog::flags::Collector::G1GC,
                max_heap_size_gb: 32.0,
                min_heap_size_gb: 32.0,
                region_size_mb: 32.0,
                target_pause_millis: 500,
                max_direct_memory_gb: 40.0,
                all_flags: vec![],
            },
            &vec![],
        );
        assert!(recs.is_empty());
    }

    #[test]
    fn test_cms_detected() {
        let recs = generate_recommendations(
            &GCFlags {
                collector: crate::glog::flags::Collector::CMS,
                max_heap_size_gb: 32.0,
                min_heap_size_gb: 32.0,
                region_size_mb: 32.0,
                target_pause_millis: 500,
                max_direct_memory_gb: 40.0,
                all_flags: vec![],
            },
            &vec![],
        );
        assert!(!recs.is_empty());
        assert!(recs.contains("CMS GC collector detected"));
    }

    #[test]
    fn test_parallel_gc_detected() {
        let recs = generate_recommendations(
            &GCFlags {
                collector: crate::glog::flags::Collector::Parallel,
                max_heap_size_gb: 32.0,
                min_heap_size_gb: 32.0,
                region_size_mb: 32.0,
                target_pause_millis: 500,
                max_direct_memory_gb: 40.0,
                all_flags: vec![],
            },
            &vec![],
        );
        assert!(!recs.is_empty());
        assert!(recs.contains("Parallel GC collector detected"));
    }
    #[test]
    fn test_serial_gc_detected() {
        let recs = generate_recommendations(
            &GCFlags {
                collector: crate::glog::flags::Collector::SerialGC,
                max_heap_size_gb: 32.0,
                min_heap_size_gb: 32.0,
                region_size_mb: 32.0,
                target_pause_millis: 500,
                max_direct_memory_gb: 40.0,
                all_flags: vec![],
            },
            &vec![],
        );
        assert!(!recs.is_empty());
        assert!(recs.contains("Serial GC collector detected"));
    }
    #[test]
    fn test_serial_zgc_detected() {
        let recs = generate_recommendations(
            &GCFlags {
                collector: crate::glog::flags::Collector::ZGC,
                max_heap_size_gb: 32.0,
                min_heap_size_gb: 32.0,
                region_size_mb: 32.0,
                target_pause_millis: 500,
                max_direct_memory_gb: 40.0,
                all_flags: vec![],
            },
            &vec![],
        );
        assert!(!recs.is_empty());
        assert!(recs.contains("ZGC GC collector detected"));
    }

    #[test]
    fn test_serial_shenandoah_detected() {
        let recs = generate_recommendations(
            &GCFlags {
                collector: crate::glog::flags::Collector::Shenandoah,
                max_heap_size_gb: 32.0,
                min_heap_size_gb: 32.0,
                region_size_mb: 32.0,
                target_pause_millis: 500,
                max_direct_memory_gb: 40.0,
                all_flags: vec![],
            },
            &vec![],
        );
        assert!(!recs.is_empty());
        assert!(recs.contains("Shenandoah GC collector detected"));
    }

    #[test]
    fn test_serial_unknown_detected() {
        let recs = generate_recommendations(
            &GCFlags {
                collector: crate::glog::flags::Collector::Unknown,
                max_heap_size_gb: 32.0,
                min_heap_size_gb: 32.0,
                region_size_mb: 32.0,
                target_pause_millis: 500,
                max_direct_memory_gb: 40.0,
                all_flags: vec![],
            },
            &vec![],
        );
        assert!(!recs.is_empty());
        assert!(recs.contains("Unknown GC collector detected"));
    }

    #[test]
    fn test_to_space_exhausted_gc_detected() {
        let pauses = vec![
            GCPause {
                attributes: vec!["young".to_string(), "to-space exhausted".to_string()],
                gc_type: "G1 Evacuation Pause".to_string(),
                is_full_gc: false,
                pause_time_seconds: 30.00,
                time_epoch: 1658739348,
                heap_sizing: HeapSizing::None,
            },
            GCPause {
                attributes: vec!["young".to_string(), "to-space exhausted".to_string()],
                gc_type: "G1 Evacuation Pause".to_string(),
                is_full_gc: false,
                pause_time_seconds: 30.00,
                time_epoch: 1658739348,
                heap_sizing: HeapSizing::None,
            },
        ];
        let recs = generate_recommendations(
            &GCFlags {
                collector: crate::glog::flags::Collector::G1GC,
                max_heap_size_gb: 32.0,
                min_heap_size_gb: 32.0,
                region_size_mb: 1.0,
                target_pause_millis: 500,
                max_direct_memory_gb: 40.0,
                all_flags: vec![],
            },
            &pauses,
        );
        assert!(!recs.is_empty());
        assert_eq!("recommendations
---------------
* 100.00% of GCs were to-space exhausted adding 60.00 total seconds pause time with a max pause time of 30.00 seconds, this means the max heap size was too small for use case during that time. Raising the heap size will help minimize the chance of this occuring again.", recs);
    }
}
