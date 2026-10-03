#!/usr/bin/env python3
"""Records transcribed from published premium tables, as fixtures for `rulec replay`.

Public data, not order data: every number here is printed in a table anyone can download,
so unlike the fixtures of docs/formats.md these files can live in the repository. Each
record is one grade of a table with the printed 折半額 (the employee's half, to the sen),
turned into the two yen amounts the same table's notes define: deducted from salary
(50銭以下は切り捨て、50銭を超える場合は切り上げ) or paid in cash (50銭未満は切り捨て、
50銭以上は切り上げ). `tests/library.rs` replays the corpus rules over these files and
requires every record to agree.

Run to rewrite the .jsonl files:

    python3 tests/oracle/make.py

The printed amounts are also checked here against 標準報酬月額 × 料率 ÷ 2, so a misread
digit in the transcription fails loudly instead of becoming a wrong expectation.
"""
import json
import pathlib
from fractions import Fraction

HERE = pathlib.Path(__file__).resolve().parent


def yen_deducted(half: Fraction) -> int:
    """給与から控除するとき: 50銭以下は切り捨て、50銭を超える場合は切り上げ."""
    q, r = divmod(half, 1)
    return int(q) + (1 if r > Fraction(1, 2) else 0)


def yen_cash(half: Fraction) -> int:
    """現金で納めるとき: 50銭未満は切り捨て、50銭以上は切り上げ."""
    q, r = divmod(half, 1)
    return int(q) + (1 if r >= Fraction(1, 2) else 0)


# 協会けんぽ 東京支部, 令和8年3月分（4月納付分）からの健康保険・厚生年金保険の保険料額表.
# 健康保険料率 9.85%（介護保険第2号被保険者に該当しない場合）, 11.47%（該当する場合）,
# 介護保険料率 1.62%. Columns: 等級, 標準報酬月額, a 報酬月額 inside the grade's range,
# printed 折半額 without care, printed 折半額 with care (円, one decimal as printed).
KENPO_TOKYO_R8 = [
    (1, 58000, 50000, "2856.5", "3326.3"),
    (2, 68000, 63000, "3349.0", "3899.8"),
    (3, 78000, 73000, "3841.5", "4473.3"),
    (4, 88000, 83000, "4334.0", "5046.8"),
    (5, 98000, 93000, "4826.5", "5620.3"),
    (6, 104000, 101000, "5122.0", "5964.4"),
    (7, 110000, 107000, "5417.5", "6308.5"),
    (8, 118000, 114000, "5811.5", "6767.3"),
    (9, 126000, 122000, "6205.5", "7226.1"),
    (10, 134000, 130000, "6599.5", "7684.9"),
    (11, 142000, 138000, "6993.5", "8143.7"),
    (12, 150000, 146000, "7387.5", "8602.5"),
    (13, 160000, 155000, "7880.0", "9176.0"),
    (14, 170000, 165000, "8372.5", "9749.5"),
    (15, 180000, 175000, "8865.0", "10323.0"),
    (16, 190000, 185000, "9357.5", "10896.5"),
    (17, 200000, 195000, "9850.0", "11470.0"),
    (18, 220000, 210000, "10835.0", "12617.0"),
    (19, 240000, 230000, "11820.0", "13764.0"),
    (20, 260000, 250000, "12805.0", "14911.0"),
    (21, 280000, 270000, "13790.0", "16058.0"),
    (22, 300000, 290000, "14775.0", "17205.0"),
    (23, 320000, 310000, "15760.0", "18352.0"),
    (24, 340000, 330000, "16745.0", "19499.0"),
    (25, 360000, 350000, "17730.0", "20646.0"),
    (26, 380000, 370000, "18715.0", "21793.0"),
    (27, 410000, 395000, "20192.5", "23513.5"),
    (28, 440000, 425000, "21670.0", "25234.0"),
    (29, 470000, 455000, "23147.5", "26954.5"),
    (30, 500000, 485000, "24625.0", "28675.0"),
    (31, 530000, 515000, "26102.5", "30395.5"),
    (32, 560000, 545000, "27580.0", "32116.0"),
    (33, 590000, 575000, "29057.5", "33836.5"),
    (34, 620000, 605000, "30535.0", "35557.0"),
    (35, 650000, 635000, "32012.5", "37277.5"),
    (36, 680000, 665000, "33490.0", "38998.0"),
    (37, 710000, 695000, "34967.5", "40718.5"),
    (38, 750000, 730000, "36937.5", "43012.5"),
    (39, 790000, 770000, "38907.5", "45306.5"),
    (40, 830000, 810000, "40877.5", "47600.5"),
    (41, 880000, 855000, "43340.0", "50468.0"),
    (42, 930000, 905000, "45802.5", "53335.5"),
    (43, 980000, 955000, "48265.0", "56203.0"),
    (44, 1030000, 1005000, "50727.5", "59070.5"),
    (45, 1090000, 1055000, "53682.5", "62511.5"),
    (46, 1150000, 1115000, "56637.5", "65952.5"),
    (47, 1210000, 1175000, "59592.5", "69393.5"),
    (48, 1270000, 1235000, "62547.5", "72834.5"),
    (49, 1330000, 1295000, "65502.5", "76275.5"),
    (50, 1390000, 1355000, "68457.5", "79716.5"),
]

# 日本年金機構, 令和2年9月分（10月納付分）からの厚生年金保険料額表（令和8年度版）, 料率 18.300%.
# Columns: 等級, 標準報酬月額, a 報酬月額 inside the grade's range, printed 折半額.
NENKIN_R8 = [
    (1, 88000, 80000, "8052.00"),
    (2, 98000, 93000, "8967.00"),
    (3, 104000, 101000, "9516.00"),
    (4, 110000, 107000, "10065.00"),
    (5, 118000, 114000, "10797.00"),
    (6, 126000, 122000, "11529.00"),
    (7, 134000, 130000, "12261.00"),
    (8, 142000, 138000, "12993.00"),
    (9, 150000, 146000, "13725.00"),
    (10, 160000, 155000, "14640.00"),
    (11, 170000, 165000, "15555.00"),
    (12, 180000, 175000, "16470.00"),
    (13, 190000, 185000, "17385.00"),
    (14, 200000, 195000, "18300.00"),
    (15, 220000, 210000, "20130.00"),
    (16, 240000, 230000, "21960.00"),
    (17, 260000, 250000, "23790.00"),
    (18, 280000, 270000, "25620.00"),
    (19, 300000, 290000, "27450.00"),
    (20, 320000, 310000, "29280.00"),
    (21, 340000, 330000, "31110.00"),
    (22, 360000, 350000, "32940.00"),
    (23, 380000, 370000, "34770.00"),
    (24, 410000, 395000, "37515.00"),
    (25, 440000, 425000, "40260.00"),
    (26, 470000, 455000, "43005.00"),
    (27, 500000, 485000, "45750.00"),
    (28, 530000, 515000, "48495.00"),
    (29, 560000, 545000, "51240.00"),
    (30, 590000, 575000, "53985.00"),
    (31, 620000, 605000, "56730.00"),
    (32, 650000, 635000, "59475.00"),
]


def check_printed(std: int, rate: Fraction, printed: str, where: str) -> Fraction:
    half = Fraction(printed)
    computed = Fraction(std) * rate / 2
    if half != computed:
        raise SystemExit(f"{where}: printed {printed} but {std} × {rate} ÷ 2 = {computed}")
    return half


def kenpo() -> str:
    out = []
    for grade, std, monthly, plain, care in KENPO_TOKYO_R8:
        for flag, printed, rate in [(False, plain, Fraction(985, 10000)), (True, care, Fraction(1147, 10000))]:
            half = check_printed(std, rate, printed, f"kenpo grade {grade} care={flag}")
            out.append(json.dumps({
                "tag": f"grade:{grade}:{'care' if flag else 'plain'}",
                "in": {"報酬月額": monthly, "健康保険料率": 985, "介護保険料率": 162, "介護該当": flag},
                "observed": {"標準報酬月額": std, "給与控除額": yen_deducted(half), "現金納付額": yen_cash(half)},
            }, ensure_ascii=False))
    return "\n".join(out) + "\n"


def nenkin() -> str:
    out = []
    for grade, std, monthly, printed in NENKIN_R8:
        half = check_printed(std, Fraction(183, 1000), printed, f"nenkin grade {grade}")
        out.append(json.dumps({
            "tag": f"grade:{grade}",
            "in": {"報酬月額": monthly, "料率": 183},
            "observed": {"標準報酬月額": std, "給与控除額": yen_deducted(half), "現金納付額": yen_cash(half)},
        }, ensure_ascii=False))
    return "\n".join(out) + "\n"


if __name__ == "__main__":
    (HERE / "健康保険料_東京_令和8年度.jsonl").write_text(kenpo(), encoding="utf-8")
    (HERE / "厚生年金保険料_令和8年度.jsonl").write_text(nenkin(), encoding="utf-8")
    print(f"wrote {len(KENPO_TOKYO_R8) * 2} + {len(NENKIN_R8)} records")
