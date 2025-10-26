# (TODO) this assumes buf is actually installed
buf:
	buf generate ./gtfs-realtime.proto --template buf.gen.yaml

