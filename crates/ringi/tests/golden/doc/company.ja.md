<!-- ringi 0.55.2 が shared/company.ringi（sha256:0b7a78fdf23df823）から生成したページです。読むためのもので、もとになるのは .ringi のほうです。ここを編集しても .ringi には戻せません。 -->
# Company

The shape of the company's organization (fictional): the positions from the lowest, the kinds of teams from the widest, and the folder that holds the data of each version

ファイル：`shared/company.ringi`（組織 `company` の版 1、sha256:0b7a78fdf23df823）

組織は、置いてある版のそれぞれ（2026-04-01、2027-04-01 から）で読んでいます。

## 役職とチームの種類

- 役職（低い順）：`staff`、`manager`、`dept_head`、`division_head`、`president`
- チームの種類（広い順）：`company`、`division`、`department`、`section`、`committee`

## データの版

データは `../org` から読んでいます。どの版も、その日から、次の版の日の前の日まで効きます。

| から | まで | チーム | 人 |
|---|---|---|---|
| 2026-04-01（水） | 2027-03-31（水） | 14 | 24 |
| 2027-04-01（木） | 終わり無し | 13 | 23 |

### 2026-04-01（水）からのチーム

| チーム | 種類 | 上のチーム | 長 | 人数 |
|---|---|---|---|---|
| 株式会社サンプル（架空）（`company`） | `company` | — | e0001（社長（架空）） | 1 |
| 営業本部（`sales_div`） | `division` | 株式会社サンプル（架空）（`company`） | e0101（営業本部長） | 1 |
| 営業一部（`sales1`） | `department` | 営業本部（`sales_div`） | e3101（営業一部長） | 1 |
| 営業一課（`sales1_a`） | `section` | 営業一部（`sales1`） | e1101（営業一課長） | 3 |
| 管理本部（`corporate_div`） | `division` | 株式会社サンプル（架空）（`company`） | e0102（管理本部長） | 1 |
| 経理部（`accounting`） | `department` | 管理本部（`corporate_div`） | e1001（経理部長） | 2 |
| 財務部（`finance`） | `department` | 管理本部（`corporate_div`） | e4001（財務部長） | 1 |
| 総務部（`general_affairs`） | `department` | 管理本部（`corporate_div`） | e5001（総務部長） | 1 |
| 人事部（`hr`） | `department` | 管理本部（`corporate_div`） | e6001（人事部長） | 1 |
| 法務部（`legal`） | `department` | 管理本部（`corporate_div`） | e9001（法務部長） | 3 |
| 情報システム部（`it`） | `department` | 管理本部（`corporate_div`） | e7001（情報システム部長） | 1 |
| 情報セキュリティ課（`it_security`） | `section` | 情報システム部（`it`） | e7101（情報セキュリティ課長） | 2 |
| 運用課（`it_operations`） | `section` | 情報システム部（`it`） | e7201（運用課長） | 2 |
| 取締役会（`board`） | `committee` | 株式会社サンプル（架空）（`company`） | e0001（社長（架空）） | 5 |

### 2026-04-01（水）からの人

| 人 | チーム | 役職 | 主に属する |
|---|---|---|---|
| e0001（社長（架空）） | 株式会社サンプル（架空）（`company`） | `president` | はい |
| e0001（社長（架空）） | 取締役会（`board`） | `president` | — |
| e0101（営業本部長） | 営業本部（`sales_div`） | `division_head` | はい |
| e0102（管理本部長） | 管理本部（`corporate_div`） | `division_head` | はい |
| e0201（取締役 1） | 取締役会（`board`） | `staff` | はい |
| e0202（取締役 2） | 取締役会（`board`） | `staff` | はい |
| e0203（取締役 3） | 取締役会（`board`） | `staff` | はい |
| e0204（取締役 4） | 取締役会（`board`） | `staff` | はい |
| e1001（経理部長） | 経理部（`accounting`） | `dept_head` | はい |
| e1002（経理担当） | 経理部（`accounting`） | `staff` | はい |
| e1101（営業一課長） | 営業一課（`sales1_a`） | `manager` | はい |
| e1201（営業担当 1） | 営業一課（`sales1_a`） | `staff` | はい |
| e1203（営業担当 3） | 営業一課（`sales1_a`） | `staff` | はい |
| e3101（営業一部長） | 営業一部（`sales1`） | `dept_head` | はい |
| e4001（財務部長） | 財務部（`finance`） | `dept_head` | はい |
| e5001（総務部長） | 総務部（`general_affairs`） | `dept_head` | はい |
| e6001（人事部長） | 人事部（`hr`） | `dept_head` | はい |
| e7001（情報システム部長） | 情報システム部（`it`） | `dept_head` | はい |
| e7101（情報セキュリティ課長） | 情報セキュリティ課（`it_security`） | `manager` | はい |
| e7102（セキュリティ技術者） | 情報セキュリティ課（`it_security`） | `staff` | はい |
| e7201（運用課長） | 運用課（`it_operations`） | `manager` | はい |
| e7202（運用担当） | 運用課（`it_operations`） | `staff` | はい |
| e9001（法務部長） | 法務部（`legal`） | `dept_head` | はい |
| e9002（弁護士 2） | 法務部（`legal`） | `staff` | はい |
| e9003（弁護士 3） | 法務部（`legal`） | `staff` | はい |

### 2027-04-01（木）からのチーム

| チーム | 種類 | 上のチーム | 長 | 人数 |
|---|---|---|---|---|
| 株式会社サンプル（架空）（`company`） | `company` | — | e0001（社長（架空）） | 1 |
| 営業本部（`sales_div`） | `division` | 株式会社サンプル（架空）（`company`） | e0101（営業本部長） | 1 |
| 営業一部（`sales1`） | `department` | 営業本部（`sales_div`） | e3101（営業一部長） | 4 |
| 管理本部（`corporate_div`） | `division` | 株式会社サンプル（架空）（`company`） | e0102（管理本部長） | 1 |
| 経理部（`accounting`） | `department` | 管理本部（`corporate_div`） | e1001（経理部長） | 2 |
| 財務部（`finance`） | `department` | 管理本部（`corporate_div`） | e4001（財務部長） | 1 |
| 総務部（`general_affairs`） | `department` | 管理本部（`corporate_div`） | e5001（総務部長） | 1 |
| 人事部（`hr`） | `department` | 管理本部（`corporate_div`） | e6001（人事部長） | 1 |
| 法務部（`legal`） | `department` | 管理本部（`corporate_div`） | — | 2 |
| 情報システム部（`it`） | `department` | 管理本部（`corporate_div`） | e7001（情報システム部長） | 1 |
| 情報セキュリティ課（`it_security`） | `section` | 情報システム部（`it`） | e7101（情報セキュリティ課長） | 2 |
| 運用課（`it_operations`） | `section` | 情報システム部（`it`） | e7201（運用課長） | 2 |
| 取締役会（`board`） | `committee` | 株式会社サンプル（架空）（`company`） | e0001（社長（架空）） | 5 |

### 2027-04-01（木）からの人

| 人 | チーム | 役職 | 主に属する |
|---|---|---|---|
| e0001（社長（架空）） | 株式会社サンプル（架空）（`company`） | `president` | はい |
| e0001（社長（架空）） | 取締役会（`board`） | `president` | — |
| e0101（営業本部長） | 営業本部（`sales_div`） | `division_head` | はい |
| e0102（管理本部長） | 管理本部（`corporate_div`） | `division_head` | はい |
| e0201（取締役 1） | 取締役会（`board`） | `staff` | はい |
| e0202（取締役 2） | 取締役会（`board`） | `staff` | はい |
| e0203（取締役 3） | 取締役会（`board`） | `staff` | はい |
| e0204（取締役 4） | 取締役会（`board`） | `staff` | はい |
| e1001（経理部長） | 経理部（`accounting`） | `dept_head` | はい |
| e1002（経理担当） | 経理部（`accounting`） | `staff` | はい |
| e1101（営業一課長） | 営業一部（`sales1`） | `manager` | はい |
| e1201（営業担当 1） | 営業一部（`sales1`） | `staff` | はい |
| e1203（営業担当 3） | 営業一部（`sales1`） | `staff` | はい |
| e3101（営業一部長） | 営業一部（`sales1`） | `dept_head` | はい |
| e4001（財務部長） | 財務部（`finance`） | `dept_head` | はい |
| e5001（総務部長） | 総務部（`general_affairs`） | `dept_head` | はい |
| e6001（人事部長） | 人事部（`hr`） | `dept_head` | はい |
| e7001（情報システム部長） | 情報システム部（`it`） | `dept_head` | はい |
| e7101（情報セキュリティ課長） | 情報セキュリティ課（`it_security`） | `manager` | はい |
| e7102（セキュリティ技術者） | 情報セキュリティ課（`it_security`） | `staff` | はい |
| e7201（運用課長） | 運用課（`it_operations`） | `manager` | はい |
| e7202（運用担当） | 運用課（`it_operations`） | `staff` | はい |
| e9002（弁護士 2） | 法務部（`legal`） | `staff` | はい |
| e9003（弁護士 3） | 法務部（`legal`） | `staff` | はい |

## 代理

データにある代理です（`../org/proxies.csv`）。承認の代理は、代わる人のステップに並んで答えます。申請の代理は、代わる人の名前で申請を出します。代理がしたことは、だれの代わりにしたかと一緒に、履歴に残ります。

| 代わられる人 | 代わる人 | 種類 | 申請の種類 | 期間 |
|---|---|---|---|---|
| e3101（営業一部長） | e1101（営業一課長） | 承認 | `expense`、`expense_ja`、`purchase`、`purchase_ja` | 2026-12-28〜2027-01-08 |
| e1203（営業担当 3） | e1201（営業担当 1） | 申請 | `leave`、`leave_ja` | 2026-12-01〜2026-12-31 |
