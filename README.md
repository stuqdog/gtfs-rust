A simple MTA gtfs-rt reader/parser, written in rust. Takes `latitude` and `longitude` as 
command line arguments, returns nearby trains (currently just the yellow lines that I use!).

Reader beware: this is very much a POC. hacky and overly specific choices abound, and they
tend to be oriented around what I personally would find useful.

`stops.txt` and other subway data found at https://rrgtfsfeeds.s3.amazonaws.com/gtfs_subway.zip

Cool potential next steps:
- generalize code (e.g., support all trains instead of just the ones I care about, maybe support other train systems?)
- customizability (e.g., allow user to override "near enough" distance/time, specify train, filter on direction, etc.)
- better document code semantics (e.g., which methods return a Result<T>, which an Option<T>, which a T, and why?)
- code hygiene - limit publicness of data members and helper functions
- handle malformed data better 
- auto download train data from Makefile if not present, and store it in a more consistent, reliable place
-- currently assume a consistent delivery format and return an error if wrong, but could try to proceed instead
- route lookup (given a starting location and a destination, determine the best train route)
- run this on a pi hooked up to a simple display so I can actually use this info
- some sort of generative art thing that uses this data would be way cool
- ensure MTA data about stops, etc. is up to date 
-- what I got was apparently updated as-of September 2025 but appears to be missing some stops
- related to the prior bullet, handle edge cases better e.g. reporting if next stop or other data is unknown
