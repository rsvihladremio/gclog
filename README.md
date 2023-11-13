# gclog
gclog is a simple fast tool that reads and analyzes gc logs for Java 8

## Status

Alpha currently only supports G1GC and JDK8 

## Quickstart

```
cargo install gclog
gclog ./gc.log

gclog 0.2.1-rustc 1.71.0 (8ede3aae2 2023-07-12)
GC Summary:
--------
OpenJDK 64-Bit Server VM (25.372-b07) for bsd-aarch64 JRE (Zulu 8.70.0.23-CA-macos-aarch64) (1.8.0_372-b07), built on Apr 18 2023 01:36:28 by "zulu_re" with gcc Apple LLVM 12.0.0 (clang-1200.0.32.28)
Total System RAM:    8.39 gb
collector:           G1GC
target pause millis: 500
region size:         1.00 mb
max heap:            2.00 gb
initial heap:        0.12 gb
max direct memory:   2.00 gb
flags:
-XX:InitialHeapSize=134217728
-XX:MaxGCPauseMillis=500
-XX:MaxHeapSize=2147483648
-XX:+PrintAdaptiveSizePolicy
-XX:+PrintGC
-XX:+PrintGCDateStamps
-XX:+PrintGCDetails
-XX:+PrintGCTimeStamps
-XX:+PrintReferenceGC
-XX:+UseCompressedClassPointers
-XX:+UseCompressedOops
-XX:+UseG1GC

Max Pause:
--------
Timestamp: 2023-11-13T08:27:06.0Z
Pause Time 10 milliseconds
Pause Type G1 Evacuation Pause
+-------------------------------+--------------+------------------+-----------+-----------+-----------+-----------+
| GC                            | Total Pauses | Total Pause Time | Min Pause | P50 Pause | P99 Pause | Max Pause |
+-------------------------------+--------------+------------------+-----------+-----------+-----------+-----------+
| G1 Evacuation Pause - (young) |      30      |       0.21       |   0.00    |   0.01    |   0.01    |   0.01    |
+-------------------------------+--------------+------------------+-----------+-----------+-----------+-----------+
```



