# Lua VM 函数调用笔记（结合当前项目实现）

## 0. 这份笔记在解决什么问题
你问的核心是：Lua 里“函数调用/返回/变参/多返回/闭包”到底怎么落到 VM。

这份项目当前已经实现了一个教学版 Lua 风格 VM，重点覆盖：
- `CALL/RETURN` 的参数与返回传播
- `VARARG`（`...`）
- `TAILCALL`（尾调用）
- `TFORCALL/TFORLOOP`（泛型 for 两阶段）
- `CLOSURE/GETUPVAL`（闭包捕获）

---

## 1. 先统一一个概念：源码没有 main，但编译后一定有可调用 chunk
- Lua 源码文件本身是 chunk，不需要你手写 `main()`。
- 但编译后 chunk 会变成一个 `Proto`（可调用函数原型）。
- 解释器执行时，本质是在调用这个 chunk 闭包。
- 在本项目里我们手写字节码，所以每个 proto 结尾要有 `return_` 指令。

---

## 2. 当前 VM 的函数调用模型（最重要）

### 2.1 统一值栈 + 寄存器窗口
- VM 不是“每个函数一条独立栈”，而是一个统一 `stack`。
- 每次调用通过 `CallFrame.base/top` 在统一栈上切一段寄存器窗口。
- `R0` 映射到 `stack[base]`。

### 2.2 调用布局约定
一次调用从 `func_index` 开始：
- `stack[func_index]`：函数对象（`LFn` 或 `Closure`）
- `stack[func_index + 1 ..]`：参数区
- callee 的 `base = func_index + 1`

### 2.3 `Vm.top` 的职责
`Vm.top` 是“有效 top”（第一个空槽位），不是 `stack.len()`：
- `B=0` 调用时，用它决定参数个数
- `Return B=0` 时，用它决定多返回个数
- `C=0` 写回后，用它更新 caller 的可见结果区间

---

## 3. 指令语义速记

## 3.1 `Call A B C`
- 函数槽位：`R[A]`
- 参数：
  - `B != 0` => `nargs = B - 1`
  - `B == 0` => 参数来自 `R[A+1]..R[top-1]`
- 返回规格：
  - `C != 0` => 固定 `C - 1` 个
  - `C == 0` => 多返回传播

### 3.2 `Return A B`
- `B != 0` => 固定返回 `B - 1` 个，从 `R[A]` 开始
- `B == 0` => 返回 `R[A]..R[top-1]`

### 3.3 `Vararg A B`
- 从当前帧 `varargs` 复制到寄存器
- `B != 0` 复制 `B-1` 个，`B == 0` 复制全部

### 3.4 `TailCall A B C`
- 参数规则和 `Call` 一样
- 不新建帧，直接用 callee 替换当前帧（尾调用优化）
- 返回规格继承当前帧，结果直接回上层 caller

### 3.5 `TForCall A C` + `TForLoop A sBx`
泛型 for 两阶段：
- `TFORCALL`：调用 `R[A](R[A+1], R[A+2])`，结果写到 `R[A+3]..`（固定 `C` 个）
- `TFORLOOP`：若 `R[A+3] != nil`，则 `R[A+2] = R[A+3]` 并跳转；否则结束循环

### 3.6 `Closure A Bx` + `GetUpval A B`
- `CLOSURE`：根据 child proto 的 upvalue 描述创建闭包值
  - `instack=true`：从当前寄存器捕获
  - `instack=false`：从当前函数 upvalues 转发捕获
- `GETUPVAL`：`R[A] = upvalue[B]`

---

## 4. 两步实现总结（你最近做的两块）

## 4.1 第一步：`TFORCALL/TFORLOOP`
你补齐了泛型 for 的 VM 语义：
- 增加了专门写回目标的调用入口（用于 `A+3` 写回）
- `TFORCALL` 正确固定接收 `C` 个结果
- `TFORLOOP` 正确执行“非 nil 才复制控制变量并跳转”

学习价值：理解“调用结果不一定写回函数槽位”，以及“控制变量更新和跳转是分开的两件事”。

## 4.2 第二步：`CLOSURE/UPVALUE`
你补齐了闭包最核心链路：
- `Value::Closure { proto_id, upvalues }`
- `Proto.upvalues` 描述捕获来源
- `CLOSURE` 负责构造闭包并冻结捕获值（当前实现是值快照）
- `GETUPVAL` 在函数体内读取闭包环境
- `CALL/TAILCALL` 已支持直接调用闭包

学习价值：把“词法作用域”转成“运行时数据结构”的关键一步。

---

## 5. 和真实 Lua 5.x 的差异（考试/面试容易问）
- 当前 upvalue 是“值快照”，不是真 Lua 的开放 upvalue（引用同一外层变量单元）。
- 没做 `SETUPVAL`、`CLOSE`、upvalue 生命周期管理。
- 没做完整元方法调用链（这里只支持 `LFn/Closure`）。

这不影响你理解主干模型，但要知道真实 Lua 会更复杂。

---

## 6. 建议按这条路径复习
1. 先把 `CALL/RETURN/top` 走通（固定返回 vs 多返回）。
2. 再理解 `VARARG`（为什么要单独指令，不直接暴露在寄存器里）。
3. 接着看 `TAILCALL`（为什么能不增栈）。
4. 最后看 `CLOSURE/GETUPVAL`（词法作用域的运行时化）。

如果你继续往下做，下一站建议是：
- `SETUPVAL`（闭包修改外层变量）
- open upvalue（外层帧没退出时共享变量单元）
- `CLOSE` 语义（外层帧退出时封闭 upvalue）

---

## 7. 两步进阶笔记（已落盘）
你刚刚学习的两步（机制图 + 逐指令追踪）已单独整理在：

- `doc/lua_vm_open_upvalue_two_steps.md`
