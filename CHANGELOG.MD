## [0.1.5] - 2022-09-21
### changed
- optimized schema algo to ignore drops without drop table if exists as there is a known bug in tableau that spams queries but has limited negative impact
- now can pass a folder to queries.json and all files will be parsed inside
- can now skip the memory report as it can be slow to generate

## [0.1.4] - 2022-09-14
### fixed
- can now parse 19.x and lower generated queries.json [ST-66](https://dremio.atlassian.net/browse/ST-66)
### Changed
- dqdrust queries-json now puts out several graphs into a single html page
- allocation divides query duration by seconds to estimate per second memory allocation. this is not perfect but should give a sense of allocation.

## [0.1.3] - 2022-09-12
### Changed
- dqdrust queries-json has changed reporting to emphasize the per second buckets as this time window has proved useful for finding what is going on in the server at the time.

## [0.1.2] - 2022-09-02
### Added
- queries-json subcommand is now active and functioning. The -o flag will generate an interactive chart, and without will get a nice console summary (console summary in beta)
### Fixed
- can parse file with no headers, not ideal mind you but it now works
- histogram was not initializing correctly, should get correct p50 and p99 now
- now parsing final seconds entry inside of a gc

## [0.1.1] - 2022-07-27
### Fixed
- reported region size in gb instead of mb, hopefully this is an obvious error and no one was affected ST-42
- accidentally reported heap expansions of 0 bytes as a heap expansion they should be skipped in the count ST-44
- we just in case filter out the heap expansion recommendation if xms == xmx ST-44 

## [0.1.0] - 2022-07-26
### Added
- dynamic version number fixes issue ST-33
- moved profile.json analysis to a subcommand
- new command gc-log subcommand is now functioning and well tested against g1gc.
- gc-log support for other GCs is very basic at this point, this will improve over time (targeting zgc, cms and shenandoah).
## Changed
- new changelog format from https://keepachangelog.com/en/1.0.0/ and post them on release this fixes ST-30
### Fixed
- in profile.json analysis was only retreiving busy scan operations with wait time but we want any time more than 0

## [0.0.4] - 2022-06-15
### Changed
- PARQUET\_WRITER special casing in recommendations
- plan phases can now skip duration in the profile.json
- Now correctly handle the Arrow Flight client which does not provide any verison information

## [0.0.3] - 2022-06-08
### Added
- initial alpha release 
- prototype phase
- have 2 recommendations in the summary
