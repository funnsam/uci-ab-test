import chess
import chess.engine
import chess.pgn
import io
import itertools

NUM_MOVES = 4
NUM_HALFMOVES = NUM_MOVES * 2
ROUGHLY_EQ_CENTIPAWNS = 75

db = open("./db.pgn")
db = db.read()
total = db.count("[Event")
db = io.StringIO(db)

fen = ""

engine = chess.engine.SimpleEngine.popen_uci("stockfish")

def analyse(game, time_limit = 0.01):
    result = engine.analyse(board, chess.engine.Limit(time=time_limit))
    return result['score']

print()

i = 0
while True:
    game = chess.pgn.read_game(db)

    if not game:
        break

    board = game.board()

    moves = 0

    for m in itertools.islice(game.mainline_moves(), NUM_HALFMOVES):
        board.push(m)
        moves += 1

    if moves < NUM_HALFMOVES:
        continue

    score = analyse(board)

    i += 1
    if abs(score.relative.score(mate_score = 10000)) < ROUGHLY_EQ_CENTIPAWNS and board.fen() not in fen:
        fen += board.fen()
        fen += "\n"

        print("\x1b[1A{}/{}".format(i, total))

db.close()
engine.close()

openings = open("../openings.txt", "w")
openings.write(fen)
openings.close()
