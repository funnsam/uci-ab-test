all: db.pgn generate

db.pgn.zst:
	wget -O db.pgn.zst https://database.lichess.org/standard/lichess_db_standard_rated_2013-01.pgn.zst

db.pgn: db.pgn.zst
	pzstd -d db.pgn.zst

generate:
	python3 ./extract.py
