# Rubash

用 Rust 从零实现的可嵌入 GNU Bash 兼容 shell 引擎。

[English](README.md) · [unixwin.dev](https://unixwin.dev)

文档与下载：[unixwin.dev](https://unixwin.dev)

[![CI](https://github.com/unixwin/rubash/actions/workflows/ci.yml/badge.svg)](https://github.com/unixwin/rubash/actions/workflows/ci.yml)
[![Rust Version](https://img.shields.io/badge/rust-1.70+-blue)](https://www.rust-lang.org)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue)](LICENSE)

## 什么是 Rubash

Rubash 是用 Rust 对 GNU Bash 语义的从零重实现，以**可嵌入的无头引擎**形态交付——词法分析、解析器、展开引擎、执行器、内建命令，全部重写。目标是与 GNU Bash 5.3.x 逐字节兼容（vendored 上游树已到 Bash-5.3 patch 20 层级，#957），原生运行在 Windows、Linux 与 macOS 上。

Rubash 本身不是 shell 产品。它自带一个参考 CLI（供兼容性 harness 和工具链使用），而交互式 shell 构建于引擎**之上**：niubash 嵌入 Rubash 承担全部 bash 语义，自己只负责行编辑、prompt 渲染与补全。

**实测而非宣称**：兼容性用 GNU Bash 自己的 83 套上游测试语料验证——2026-10-12 全量重跑 83/83 全部通过。Windows 上原始 stdout 逐字节相等为 62/83；另有 20 套仅在环境绑定行上有差异（宿主工具输出、NTFS 文件名/属性位、locale、路径形式——台账逐行归因），nameref 的唯一残差行经 GNU 自我对跑证实为 GNU 自身 coproc 收割时序竞争、非 rubash 分歧（台账见下）。

**原生的意义**：打着"Windows 上的 bash"旗号的方案（Git Bash、MSYS2）装的是移植版 bash，骑在 POSIX 模拟层（`msys-2.0.dll`）上——fork 模拟、路径翻译的怪癖会渗进每一个脚本。Rubash 没有这层：一个自包含二进制，直接对话 Win32。

**路径是一等公民，一个子进程一种方言**：MSYS 的模型是*猜*哪些参数像路径然后改写——这就是为什么每个 AI agent 和脚本都得设置 `MSYS_NO_PATHCONV=1`，防止 `/flags` 被改成 `C:/Program Files/Git/flags`。Rubash 用**逐子进程方言契约**把模型反过来（与 GNU 画的是同一条线：`shell_execve` 把原始词字节原样交给 `execve`，从不改写 argv）。自带 POSIX 层的子进程——凡是从 WinuxCmd 安装解析出来的、或经 `winuxcmd` 分发器解析的——拿到的 argv **逐字原样**，`/d/...`、`/tmp`、`/dev/*` 由它自己解析。原生 Windows 程序则拿到路径形参数的真实 Win32 拼写，翻译**整体一致**（绝不半个 argv 被翻译、另一半留在 POSIX 形），原生工具的每个操作数——包括它即将创建的尚不存在目标——都拿到合法 Win32 路径。紧急回退：导出 `__RUBASH_ARGV_DIALECT=legacy` 可恢复 Option-B 之前的翻译行为。

**平台状态——跨平台、Windows 优先**：Windows 是主战场，拥有完整技术栈：单一自包含二进制直面 Windows 用户的 bash 痛点（无 POSIX 模拟层、无路径转换猜测、无需 MSYS_NO_PATHCONV、原生 Win32 路径为通货、为 bash 生态提供 MSYS2 兼容身份）。Linux 原生构建并运行（x86_64-unknown-linux-gnu）——真 getrlimit/chmod/faccessat/uname(2) 语义与信号投递——CI 在 Ubuntu 上跑引擎测试车道（库测试、兼容性 `cli_tests`、marker goldens），并且每次推送都回放 GNU 上游语料（`Bash upstream test progress` job，`scripts/run-bash-upstream-tests.sh`）；更早在 Linux 二进制上人工跑同一语料的成绩为 19/24 套字节一致（2026-09-26，#500），剩余差距在 issue tracker 跟踪。macOS 在 CI 跑库测试套件（`macos-latest`），另有 cross-target check。引擎语义模型刻意平台中立，账本随行。

## 身份与兼容性（rubash#154）

引擎提供**可选的平台身份**，而不是把它外包给 PATH 上恰好躺着的那个 `uname.exe`。`uname` 与 `arch` 是引擎内建命令（完整选项解析，coreutils/MSYS2 输出形状），`OSTYPE`/`MACHTYPE`/`HOSTTYPE` 跟随同一人设。

- **默认人设：MSYS2 兼容**（Windows 默认；处处披露——这是兼容性面具，不是"MSYS2 移植版"的宣称。非 Windows 构建永远报诚实原生身份——MSYS 人设的存在意义是把 Windows 留在 MSYS/Cygwin 脚本生态里，其他平台不可达）：
  - `uname -s` → `MSYS_NT-<ver>`——设了 `MSYSTEM` 时按 MSYS2 运行时的同样映射规则给 `MINGW64_NT-` / `UCRT64_NT-` / `CLANG64_NT-` …；
  - `uname -m` / `arch` → `x86_64` / `aarch64`（构建架构）；
  - `uname -r` → Windows 版本串（`10.0-19044`，与 `uname -s` 结尾同一串）；
  - `uname -o` → `Msys`；`uname -a` → `sysname nodename release version machine Msys`（Git Bash 形状）；
  - `OSTYPE=msys`、`MACHTYPE=<arch>-pc-msys`（`set_if_not` 风格绑定，同 GNU variables.c:723-725——继承值优先）。
  按 `case "$(uname -s)" in MINGW*|MSYS*|CYGWIN*)` 或 `$OSTYPE` ∈ {msys, cygwin} 分叉的生态脚本，走它们测试最充分的那条路。
- **`RUBASH_IDENTITY=native` 切到诚实原生人设**：`uname -s` → `Windows_NT`（原生 `%OS%` 值），`uname -o` → `Windows`，`OSTYPE=windows`，`MACHTYPE=<arch>-pc-windows`。测试与原生优先的用户选这个；unset 该变量（或设为其他任何值）即回默认。

**披露面**（人设绝不沉默）：

- `rubash --help` 打印 Identity 段（当前人设 + 切换方法）；
- `rubash --identity` 打印人设与每个生效值（`uname -s/-m/-r/-o`、`arch`、`OSTYPE`、`MACHTYPE`）；
- 本 README 的这一节与仓库根 `SKILL.md` 的披露节（让消费本仓的 AI agent 也知道人设语义）。

`uname`/`arch` 是*隐藏的* fast-path 内建（同 `sleep`/`dirname`）：`type`/`enable`/`compgen -b` 照报外部命令，且 PATH 上一个 `uname.exe` 都没有时照样能跑。

## 兼容性一览

```
GNU Bash 5.3.x 测试套件 — 83 个文件，true-baseline 实测
（台账：2026-10-12 全量重跑，master 764b9dde，已剥离宿主 WINUXSH_ROOT
 钉扎——无 upstream 脚本桩、niu 挂载 /bin/sh 夹具、逐套件 TMPDIR 隔离、
 前台进程组超时；环境绑定差异经逐行审计后归零计；更早切片：
 2026-09-25，71c933eb）

  通过：          83/83 套件  ██████████████████████████████  100%
  ────────────────────────────────────────────────────────────────
  原始值：        62/83 套件 Windows 上 stdout 逐字节一致——
                 字面 0 差异的运行，scripts/true-baseline.sh
  环境归类：      +20 套仅在环境绑定行上有差异（宿主工具输出、
                 NTFS 文件名/属性位、locale、/dev/tty、路径形式、
                 信号表、coproc 收割时序）——台账逐行归因见下
  净化值：        82/83 套经逐行环境审计后无 rubash 引起的差异
  nameref：       1 行（coproc 后 declare -r 可见 RO_PID）——证实为
                 GNU 5.3.0 自身 coproc 收割时序竞争：GNU 自我对跑在
                 harness 文件系统上 59/60↔1/60 翻转，ext4 上 60/60
                 确定性且与 rubash 输出逐字节一致（stdout 与 stderr）
                 ——docs/nameref-suite-residual-gnu-race-20261012.md
  9 月 9 日为 3427 行 → −99%+
```

### 完全通过的套件（零差异，环境差除外）

净化套件集派生自台账的逐套件分类，并非手工维护：仅当某套件的每一差异行都归因于环境（按 AGENTS.md 的 rubash 引起 vs 环境引起划分，并对应覆盖 issue）时才列入此处。上方原始计数（62）+ 环境归类集（20）+ nameref 已证实的 GNU 竞争 = 全部 83 套。

`alias` `appendop` `arith` `arith-for` `array` `assoc` `attr` `braces` `builtins` `case` `casemod` `complete` `comsub-eof` `comsub-posix` `comsub` `comsub2` `cond` `coproc` `cprint` `dbg-support` `dbg-support2` `dstack` `dstack2` `dynvar` `errors` `exp` `exportfunc` `extglob` `extglob2` `extglob3` `func` `getopts` `glob-bracket` `glob` `globstar` `heredoc` `herestr` `histexp` `history` `ifs-posix` `ifs` `intl` `invert` `invocation` `iquote` `jobs` `lastpipe` `mapfile` `more-exp` `new-exp` `nquote` `nquote1` `nquote2` `nquote3` `nquote4` `nquote5` `parser` `posix2` `posixexp` `posixexp2` `posixpat` `posixpipe` `precedence` `printf` `procsub` `quote` `quotearray` `read` `redir` `rhs-exp` `rsh` `set-e` `set-x` `shopt` `strip` `test` `tilde` `tilde2` `trap` `type` `varenv` `vredir`

（第 83 套 nameref 见上方台账——其唯一残差行经逐字节证实归因 GNU 自身 coproc 收割竞争，见 `docs/nameref-suite-residual-gnu-race-20261012.md`。）

### GNU 套件之外——混合评测套件与真实生态

同一 true-baseline 方法（脚本文件、对 GNU 5.3.x 逐字节 diff）也用在专门评测套件与真实语料上（完整清单与裁决归档于 issue #477）：

| 面 | 结果 |
| --- | --- |
| **modernish** 能力套件（跨 shell 怪癖探针） | **166/166 rc 向量一致** |
| **mvdan/sh 解析语料** | 559 个片段，操作符类修复后 parse-accept/exit 对齐 |
| **ble.sh**（29,601 行行编辑框架） | `bash -n` **rc=0 对齐**；source 模式可终止 |
| **bash-completion** | 452/452 矩阵；`bash -n` 金丝雀逐字节一致 |
| **git-completion** | 140/140 |
| **oh-my-bash** | 22 个 lib 加载，**252/252 函数**环境对齐，PS1 逐字节一致（5 款主题） |
| **nvm.sh**（v0.40.x loader） | 加载逐字节干净，含 Windows 原生空 `NVM_DIR` 自动使用（`$PWD` POSIX 人设，#537）；自有测试切片的引擎侧失败已修 |
| **FFmpeg / OpenSSH / PHP / ltmain / cmake configure 脚本** | `--help`/错误路径输出**逐字节一致** |
| **git 自家测试框架**（t/test-lib.sh） | 在引擎下端到端运行 |
| **GNU 83 套件 · Linux 原生二进制** | 19/24 套字节一致（2026-09-26 那一轮，#500）；该语料现由 CI 每次推送回放（`Bash upstream test progress`，Ubuntu） |

以上全部钉为回归夹具（`tests/` 下 335 个集成测试文件，含 `tests/regression.rs`——GNU golden 矩阵、套件切片快照、perf 金丝雀），CI 每次推送都跑——外加一道 C 源码逐行审计（`docs/SOURCE-AUDIT.md`，已盘点 336 个行为），总在套件踩到之前把缺口找到并关掉。

## 性能

实测而非宣称——下面每个数字都钉在产出它的 issue/PR 上，守住它的测试或门禁也一并写明。仓内台账在 [`docs/performance.md`](docs/performance.md)；perf 金丝雀随 CI 回归套件跑（`cargo test --test regression`）。

**解释器热路径——Android 矩阵的三刀归因，带数字关闭。** niubash#352 的 Android 基准（aarch64，对 GNU bash 5.3.20）把解释器侧差距归因到三个子系统；三者全部修完关闭：

- 算术 for / 循环执行 2.37x → 经 #665/#671/#679 关闭（rubash#607）；
- 参数扩展单操作最高 7.3x → #666 后仅 `${s^}` 一项即 ~6x（rubash#608）；
- 标量赋值 2.72x → #672/#678 后对 GNU 净 ~1.06x（rubash#609）。

**数组存储：O(n²) → O(1) 均摊**（#618/#616）。元素写（`m[k]=$i` / `a[i]=$i`）改为追加存储段而非整串重渲染，预算触发压缩——5000 个不同键时，assoc 写入对 GNU 从 445x → 9.9x，indexed 从 736x → 3.8x（台账：`docs/performance.md`）。

**字面量 fast-path 波次**（每刀带实测差值与 GNU 差分电池）：

- 多词字面量命令 fast path：`[ 1 -eq 2 ]` ×20k ≈177→155 ms（#967）；
- `[[ ]]` 数值叶直接处理 + 十进制 fast path：50k `[[ 1 -eq 2 ]]` 对 GNU 从 1.41x→1.10x（#966）；
- procsub 元数据扫描白名单跳过：1.56x→1.39x（~11%）（#974）；
- 十进制下标数组写 fast lane：交错 8k 从 3.65x→3.16x；顺手修掉 `a[$((i++))]=v` 的真实双副作用（#959）；
- case：`$BASH_COMMAND` 只渲染头臂：20 臂 ×10k −10%，附分歧修复（#958）；
- 字面量词准入在 `expand_word_mut` 与命令词间共享，瞄准 export/unset 主体里 43% 的 `expand_command_words` 份额（#956）；同族还有 walker entry-chain 修剪（#973）与 `shift` 的偏移窗位置参数（#954）。

**export/unset 经济学**——[issue #916](https://github.com/unixwin/rubash/issues/916) 实测了该家族的单操作成本（如 2000 次动态 export：245 ms 对 GNU 33 ms；unset 腿约 200x）。写车道修复已合入（#917/#918/#920：单探针镜像、门控 env 回退、批量去标记；#943 对 unbind 做正确性返工）；issue 对残留保持开放。

**Windows 进程经济学。** 无 console 会话过去每次外部 spawn 都要付一次隐藏 console 分配，~22 ms/子进程（niubash#462）。#913 改为一个会话一个隐藏 console——Windows 11 24H2+/Server 2025 上预分配无窗 console，其他系统认领首个 `CREATE_NO_WINDOW` 子进程的——由 `tests/console_window_gate.rs` 的 14 条回归测试钉死。tput 白名单臂（#884，[#800](https://github.com/unixwin/rubash/issues/800) 的 A 面）对准入的 TERM×能力形状进程内作答，269 行差分矩阵与 GNU ncurses 字节相等：票面探针（64 次 tput 调用）从 1165 ms → 47 ms，零 spawn，由 `tests/issue800_tput_inline_arm_regressions.rs` 钉死。winuxcmd 侧的同族修复去掉了真二进制里 ~17 ms/次 的 i18n 开销（修后进程内 ~0.4 ms；3208 行回放逐字节一致，[WinuxCmd#1269](https://github.com/unixwin/WinuxCmd/pull/1269)）。#800 的 B 面——`read` 撞 EOF ~4.8 s/次——仍然开放。

## 架构

```
src/
├── bin/             `rubash` 参考 CLI
├── lexer/           词法分析器（引号、转义、heredoc、续行）
├── parser/          递归下降（简单命令、管道、case、arith-for、[[ ]]）
├── expand/          展开引擎（参数、glob、花括号、tilde、算术）
├── executor/        命令执行、内建命令、分发、数组、trap、身份
├── builtins/        62 个引擎内建（declare、read、printf、kill、...）
├── fd/              fd 表与传输层
├── jobs/            作业控制、coproc、进程替换
├── shell/           ShellState、子壳克隆、初始化
└── lib.rs           核心类型和错误处理
```

- **词法分析器**：Bash 风格引号、转义、注释、变量、命令替换、算术展开、here-doc/here-string token、常见重定向。
- **解析器**：简单命令、管道、AND/OR 列表、函数、花括号/子 shell 组、`if`、`for`、算术 `for`、`while`、`until`、`case`、`select`、`[[ ... ]]`、`coproc`、`time` 前缀。
- **执行器**：外部命令、管道、重定向、临时赋值、函数调用、`source`/`.`、`eval`、无 shebang 脚本回退，以及逐子进程 argv 方言契约（POSIX 层子进程拿逐字 argv，原生子进程拿一致翻译的 Win32 拼写）。
- **展开系统**：变量、位置参数、索引/关联数组、命令替换、算术展开、花括号展开、tilde 展开、路径名 glob、`${parameter...}` 操作符、大小写/替换变换。
- **内建命令**：62 个引擎内建——机器核对的清单在 [`docs/builtins.md`](docs/builtins.md)（由 `src/executor/builtin_names.rs` 上的测试守护同步），外加有意隐藏的 fast-path 内建（`sleep`、`dirname`、`basename`、`uname`、`arch`），让 `type`/`compgen -b` 照报外部命令，与 GNU 完全一致。

### 无 fork 的子壳语义

POSIX `fork()` 没有 Win32 等价物。仿真层（MSYS2、Cygwin）在系统调用层面硬造它——昂贵、脆弱，也正是它们最出名怪癖的来源。Rubash 改为在**语义**层面复刻 fork，分三层：

1. **进程内子壳**。`( list )` 和 `$( )` 从不创建进程。`ShellState::clone` 产出子壳的变量、别名、函数、trap 和历史——即 fork 的内存侧语义——子壳结束时副本直接丢弃。
2. **带 POSIX `dup` 语义的真实句柄 fd 表**。槽位持有原生 Windows `HANDLE`，而 `DuplicateHandle` 复制的句柄共享同一内核文件对象——因此共享同一文件偏移。这正是 POSIX "dup 共享 open file description" 的语义，落地前在 POC 中实证过。`fork_table()` 逐句柄复制整表，正如 `fork` 复制 fd 表而不复制文件对象。
3. **真实进程只出现在真实进程边界**。外部命令与管道成员走 `CreateProcess` + `os_pipe`（Unix 上原生 `fork`/`exec`）；后台作业与 coproc 拥有独立进程，作业控制基于 Job Objects，Windows 上 SIGCONT 通过 `ResumeThread` 投递。

结果：子壳与命令替换的创建成本为零，而脚本可观测的一切——退出码、fd 继承、共享偏移、信号处置——都与 GNU Bash 一致。

## 快速开始

### 从源码构建

> 跨平台：Windows 是全栈主平台；Linux 原生构建并运行，带 CI 测试车道；macOS 在 CI 跑库测试套件（见上方「平台状态」）。

```bash
git clone https://github.com/unixwin/rubash.git
cd rubash
cargo build
target/debug/rubash --version
```

### 运行脚本

```bash
target/debug/rubash path/to/script.sh
target/debug/rubash -c 'echo hello from rubash'
```

### 运行兼容性测试套件

```sh
# GNU 上游语料回放（CI 车道运行器，Linux）：
scripts/run-bash-upstream-tests.sh        # 写 target/bash-upstream-tests/summary.md

# 全量 83 套件 true-baseline 测量（台账背后的 harness；
# Windows 侧用法照 scripts/true-baseline.sh 文件头）：
MSYS_NO_PATHCONV=1 wsl bash /path/to/rubash/scripts/true-baseline.sh          # 全部 83 套
MSYS_NO_PATHCONV=1 wsl bash /path/to/rubash/scripts/true-baseline.sh array    # 单个套件
```

## 测试

```bash
# 单元 + 集成测试（tests/ 下 335 个集成测试文件）
cargo test --lib
cargo test --test cli_tests

# 回归套件：GNU golden、生态快照、perf 金丝雀
cargo test --test regression

# bashdb 兼容性
cargo test --test cli_tests bashdb_compat -- --nocapture

# source 展开
cargo test --test cli_tests source_expands -- --nocapture

# 无 console 会话门禁（一个会话一个隐藏 console，零可见窗）
cargo test --test console_window_gate
```

引擎还可以端到端运行 [bashdb](https://github.com/Trepan-Debuggers/bashdb) 核心调试循环（list、step、next、where、continue、quit）。

## 文档

- GitHub issue tracker — **唯一权威来源**，Rubash ↔ GNU Bash 兼容性状态（快照台账已于 2026-10-10 归档至 issue #477）
- [`docs/PROVENANCE.md`](docs/PROVENANCE.md) — 来源声明：Rubash 与 GNU Bash 源码的关系，以及贡献者方法论规范
- [`docs/builtins.md`](docs/builtins.md) — 内建命令清单和分发模型
- [`docs/bashdb-debugging-rubash.md`](docs/bashdb-debugging-rubash.md) — bashdb fixture 设置和 smoke test
- [`docs/bash-upstream-tests.md`](docs/bash-upstream-tests.md) — 如何运行 GNU Bash 上游测试

## 开发原则

- 按 Bash 语义的 root cause 修 Rubash 子系统，不按单条 expected output 打补丁。
- bashdb 保持外部 clean 工具；临时 instrumentation 仅用于诊断。
- 每个失败的 bashdb 命令都是发现和修复 Rubash 兼容性缺口的机会。
- 兼容性基线为 GNU Bash 5.3.x——vendored 上游树（`third_party/bash`）在 Bash-5.3 patch 20 层级（#957），差分 oracle 为本地与 WSL 的 GNU 5.3.20 基线（#978）。

## 来源与实现方式

Rubash 是对 GNU Bash 语义的 **Rust 从零重写（rewrite）**——不是对 GNU Bash C 代码的移植或翻译。GNU Bash 的任何源码都没有被编译进、链接进或复制进 Rubash 自身代码。兼容性以 GNU Bash 5.3.x 的**可观测行为**为定义，并通过黑盒差分测试验证。vendored 的 GNU Bash 源码位于独立的 `third_party/bash` 子模块，保持其原始 GPL-3.0-or-later 许可证，仅作为语义参考与测试 oracle 使用。完整声明与贡献者规范见 [`docs/PROVENANCE.md`](docs/PROVENANCE.md)。

## 许可证

MIT — 详见 [`LICENSE`](LICENSE)。

## 贡献

欢迎提交 issue、兼容性复现、focused regression tests 和实现补丁。贡献前请阅读 [`AGENTS.md`](AGENTS.md)。

## 致谢

- GNU Bash 团队 — 参考实现，其可观测行为定义了我们的兼容性目标
- Trepan-Debuggers/bashdb — 外部调试器和兼容性压力测试
- Rust 社区 — 语言和工具链

---

*最后更新：2026-10-19*
