from bloom_filter2 import BloomFilter
import chess
import chess.engine
import chess.pgn
import chess.polyglot
import io
import itertools

NUM_MOVES = 6
NUM_HALFMOVES = NUM_MOVES * 2
ROUGHLY_EQ_CENTIPAWNS = 75

EXTRACT_POSITIONS = 4096

db = open("./db.pgn")
total = 179550

fen = ""

def hasher(bloom_filter, key):
    for probeno in range(1, bloom_filter.num_probes_k + 1):
        yield key % bloom_filter.num_bits_m
        key = int(key / bloom_filter.num_bits_m)

visited = BloomFilter(max_elements = total, error_rate = 0.1, probe_bitnoer = hasher)

engine = chess.engine.SimpleEngine.popen_uci("stockfish")

def analyse(game, time_limit = 0.01):
    result = engine.analyse(board, chess.engine.Limit(time=time_limit))
    return result['score']

print(total)

i = 0
j = 0

while True:
    game = chess.pgn.read_game(db)

    if not game:
        break

    board = game.board()

    moves = 0

    for m in itertools.islice(game.mainline_moves(), NUM_HALFMOVES):
        board.push(m)
        moves += 1

    h = chess.polyglot.zobrist_hash(board)

    i += 1

    if moves < NUM_HALFMOVES or h in visited:
        continue

    visited.add(h)

    score = analyse(board)

    if abs(score.relative.score(mate_score = 10000)) < ROUGHLY_EQ_CENTIPAWNS:
        fen += board.fen()
        fen += "\n"

        j += 1

        print("\x1b[1A{}/{} ({:.2f}%) extracted {}".format(i, total, 100 * i / total, j))

        if j >= EXTRACT_POSITIONS:
            break

db.close()
engine.close()

openings = open("../openings.txt", "w")
openings.write(fen)
openings.close()
