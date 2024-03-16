import chess
import chess.engine
import chess.pgn
import itertools

db = open("./db.pgn")
fen = ""

engine = chess.engine.SimpleEngine.popen_uci("stockfish")

def analyse(game, time_limit = 0.01):
    result = engine.analyse(board, chess.engine.Limit(time=time_limit))
    return result['score']

print()

total = db.count("[Event")

i = 0
while True:
    game = chess.pgn.read_game(db)

    if not game:
        break

    board = game.board()

    moves = 0

    for m in itertools.islice(game.mainline_moves(), 8):
        board.push(m)
        moves += 1

    if moves < 8:
        continue

    score = analyse(board)

    i += 1
    if abs(score.relative.score(mate_score = 10000)) < 100 and board.fen() not in fen:
        fen += board.fen()
        fen += "\n"

        print("\x1b[1A{}/{}".format(i, total))

        # if i >= 2048:
        #     break

db.close()
engine.close()

openings = open("../openings.txt", "w")
openings.write(fen)
openings.close()
