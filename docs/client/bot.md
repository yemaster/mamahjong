# 牌效机器人

状态：四麻、三麻可用
最后更新：2026-07-31

## 边界

`clients/bot` 是独立进程，只调用 `/api/v1`：

- 注册测试账号；
- 创建东风测试房、加入、准备和开局；
- 读取观察者视图；
- 提交版本化牌局命令；
- 打到整场结算并输出统计。

机器人不依赖 `apps/server`、`mamahjong-application`、`mahjong-core` 或
`mahjong-riichi`。服务端仍是合法动作、和牌和计分的唯一裁定者。

## 打牌策略

每种可打牌张分别计算：

1. 一般形、七对子和国士无双的最小向听数；
2. 能降低向听数的有效牌；
3. 根据当前可见牌扣除后的受入枚数。

排序顺序为：

```text
向听数低 → 受入枚数多 → 有效牌种类多 → 连接度低的牌先切
```

同种五牌保持相同结构计数；完全同等时保留赤五。三麻不会把二万至八万计入
受入。立直后只摸切；可和牌时优先自摸或荣和。

副露只在调用后向听数严格低于不副露时执行。候选之间先比较向听数和受入，
再按碰、明杠、吃排序。最终并发优先级仍由服务端按
“荣和 > 碰/明杠 > 吃”裁定。

算法采用日麻牌效常用的“向听数优先、受入枚数次优”定义，参考
[Mahjong Fundamentals：Shanten 与 Ukeire](https://mahjong.guide/2018/01/06/mahjong-fundamentals-3-basic-tile-efficiency/)、
[Tenhou 牌效计算器说明](https://mahjong.guide/2017/03/18/mahjong-tools-1-tenhous-efficiency-calculator/)
以及向听数算法论文
[A Fast Algorithm for Computing the Deficiency Number of a Mahjong Hand](https://arxiv.org/abs/2108.06832)。

## 用法

```bash
# 四麻和三麻各跑一场
cargo run -p mamahjong-bot -- --all

# 单独测试
cargo run -p mamahjong-bot -- --variant yonma
cargo run -p mamahjong-bot -- --variant sanma

# 减少输出
cargo run -p mamahjong-bot -- --all --quiet
```

可用参数：

```text
--server URL
--all
--variant yonma|sanma
--max-commands N
--quiet
```

这是进攻型回归机器人，不实现读牌、防守、押引和打点期望值，不作为高水平
竞技 AI。
