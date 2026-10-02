# M1 Concept Questions ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------
1. eval_expr returns Result<Value, Control>. Ok(v) means the expression means the expression returned a value. A stuck expression is when it cant further evaluate a step because it violates a runtime invariant the checker didnt catch. Stuck expressions come back as Err(Control) where the underlying error is a RuntimeError variant converted via .into(). For a type error, the RuntimeError::TypeError contains the type that was expected and the type that was actually found, along with the span of the operation that caused the error.

2. in Expr::Unary let v = self.ecal_xpr(sub, env)?; if evaluating sub gets stuck, it returns Err(Control::...), and ? immediately returns that Err from the outermost call, the unary arm never reaches its match (op, v). Without a ? it would look like:
    let v = match self.ecal_expr(sub, env) {
        Ok(v) => v,
        Err(e) => return Err(e),
    };

3. In my and / or implementation it evaluates lhs first and uses a match to decide wheather the rhs needs to be evauluated. For and, the right side is only evaluated whent he left side is true. For or the right side is only evaluated when the left side is false. The rhs self.eval.expr call is inside the appropriate match branch, so it is skipped when the left side already determines th result.

4. Add, Sub, Mul, Div, and Mod use wrapping_add, wrapping_sub, and so on. Using wrapping methods makes the overflow a defined behavior instead of a lanugage panic that would crash the interpreter. The wrapping behavior is explicityly requested by using theses wrapping methods,so it doesnt depend on compiler settings.

5. every stuck expression uses span: *span, where span belongs to the enclosing Expr node being evaluated. the Expr::Binary arm uses its own span when a type error occurs. This is useful because the span points to the operation that caused the runtime error instead of pointing to a larger enclosing expression or only to a sub expression

# How I implemented it

1. In my Expr::Binary arm and / or are peeled off first and handled with short-circuiting match / return, since after this evaluation is safe. For all other operators both operators are evaluated: let l = self.eval_expr(lhs, env)?; let r = self.eval_expr(rhs, env)?; using ? twice here so that a stuck lhs shor circuits before the rhs is even looked at. Eq/Ne just compare the values with == and !=. The int only group does one match (&l, &r) to pull out ints or bail with a type error for whichever side want an int, then a second match op dispatches to the wrapping arithmetic with Div/Mod checking if b == 0 first before returning. Concat matches (l, r) structurally, Str + str builds a new Rc<str>, List + List calls .concat() and anything else is a type error. Cons checks that the right operand is a list and if it is it calls List::cons(l, tail) to put the left value at the front, otherwise a type error will be reported. Also every error path ends in into() to lift a runtime error into whatever COntrol wraps it, the ? or return gets it out of eval_expr.

2. The two trickest bits for me where the and / or short circuiting because my initial mistake was evaluating both sides before checking, as well as forgetting to check if a value was zero before the wrapping_div or wrapping_rem call.



# M2 Concept Questions ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------
1. env.lookup(x) returns Option<Value>. The two match arms are Some and None, some returns the value, while none returns an UnboundVariable error. For the stuck outcome, the variant I build is UnboundVariable, its fields are name and span, and it blames the Var expression's own span from Expr::Var(name, span) not some other node's span.

2. My block arm threads the environment through its items when it hits Stmt::Let(name, _ty, rhs, _), it calls self.eval_expr(rhs, &env) which is the environment as it stood before the let. Then env = env.extend(name.clone(), v) is where i call env.extend (line 238). The environment th enext iteration of the loop sees is the extended one.

3. The env variable inside the block arm is a local variable of that function call. extend builds a new environment and returns it, the env passed in as the &env parameter is never mutated. So when the blocks eval_expr call returns, that local env binding is dropped. The blocks own env variable goes out of scope when the function returns

4. This produces 2 rather than a mutation of the original binding because the second let's rhs looks up x in the environment as it exists before the second let binds, where x is still bound to 1. The result of that becomes a new fram layered on top of that instead of overwriting the old frame in place. M2's let cant build a counter whose value changes over time because its just shadowing the original binding instead of using a Value::Ref where a single cells value can actually change instead of shadowing the first binding.

5. For {z; 1} z is a Stmt:Expr, so my arm calls self.eval_expr(e, &env)? on it. z is Expr::Var("z", span_of_z), so that inner call returns the UnboundVariable error. The ? immediately returns the UnboundVariable error from the whole block arm and the loop never reaches the 1 item or the tail. The span reported error is z's own span from Expr::Var(_, span) not the blocks span.

# How I implemented it
1. Expr::Block(stms, tail, _span) is pattern matching its checking if this Expr a Block variant and if it is, it unpacks the tree fields into three new local variables. let mut env = env.clone(); shadows the parameter so theres now a local variable called env thats seprate from the env: &env parameter, and its declared mut because it gets reassinged inisde the loop. In for stmt in stmts, stmts is a &Vec<Stmt> so this loop does one &Stmt at a time, in order. The innner match stmt is an enum with two variants(let and expr), so the match dispathes on which kind of statement its looking at. let v = self.eval_expr(rhs, &env)? calls eval_expr recursively on the rhs evaluating it in the current environment and returns a Result<Value, Control>. env.extend(name.clone(), v); is a reasignment not a mutation of the old environments contents. Sstmt::Expr(e) => {self.eval_expr(e, &env)?} is the same idea as when ? is used above but the resulting value is thrown away. After the loop, tail is &Option<Box<Expr>>, and Some(t) means there is a trailing expression and it evaluates the tail in the final env, while None means there is no trailing expression and returns OK(Value::Unit). my Stmt::Let(name, _ty, rhs, _let_span) ignores the Option<Ty> using _ty.

2. A tricky bit for me was I initially tried to lookup and overwrite rather than extend and rebind. I also forgot the ? operator on the Stmt::Expr case and had the example {z; 1} case get stuck silently. I used an assistant to help me find the bugs and point me in the right direction of solving it, and I checkied its work by tracing the stuck example by hand.




# M3 Concept Questions ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------
Your eval_expr returns Result<Value, Control>, and a return travels as Err(Control::Return(v)) — the same Err side an error uses. In your Expr::Return arm, how do you build that value, and why carry a return as an Err rather than an ordinary Value? How do the two channels — Control::Return and Control::Raise — stay distinct, and what catches Control::Return at this milestone?

1. The ? propagates the error with evaluating e to the top of the call stack before it even gets to building the return value. Once I have v I wrap it in Err(Control::Return(v)) instead of Ok(v) because the return carries the value out of of a potential loop. The two channels stay distinct because Control is an enum with at least two variants, Raise and Return. They are different variants of the same enum, so something that explicitly wants to handle one differently from the other has to match on which variant it got, which my code hasnt implemented yet. The outermost function catches Control::Return at this milestone.

Walk through your assignment arm e1 := e2: in what order do you evaluate the two sides, which one must be a Value::Ref(loc), which self.store operation you call, and what value the form returns. For the stuck case — assigning to something that is not a reference — name the RuntimeError variant you build and how you form the expected type.

2. I evaluate e1/LHS first where I call self.eval_expr on it, Then e2 is evaluated afterwards. The RHS has to come back as Value::Ref(loc) to proceed, I call the self.store.write operation and I pass it (loc, v). On success the arm returns a Value::Unit. The RuntimeError variant I build is the TypeERror variant, and I form the expected type with Ty::reference(Ty::meta(0)) since I dont know what type is supposed to be behind the reference.

The Mutation chapter's rules thread a store in and a store out, yet your interpreter keeps one self.store and mutates it in place. Explain, using let a = ref 1; let b = a; b := 9; deref a ⇒ 9, why aliasing works: what ref allocated, what b = a copied, and why writing through b is visible through a.

3. let a = ref 1 creates a new store with self.store.alloc and a ends up being bound to b in the second let. b = a writes to the loc referenced by a. deref a reads from the loc associated with let b = a, hence writing through b is visible through a.

Your while arm is a host loop and your for arm walks a list. For each, say what value the body produces and what you do with it, what the loop as a whole evaluates to, and — for for — how env.extend(x, item) gives the loop variable a fresh binding each turn. Then explain how a return inside either body escapes the loop, referring to the ? in your arm.

4. In my while arm the body's value is kept after every iteration, the loop as a whole returns a Err(RuntimeError:TypeError) once the condition goes false. In my for arm env.extend(name.clone(), item.clone()) extends a new environment and therefore gives the loops variable a fresh binding each turn by cloning the name and item into that environment. A return inside either body escapes the loop by returning to the nearest enclosing function after self.eval_expr(body, &loop_env)?;. 

In if 3 { 1 } else { 2 } the condition is not a Bool, and the program is stuck. Walk through how your Expr::If arm produces that, and contrast it with if false { 9 }, which is not stuck. Which branches does your arm evaluate in each case, and what does an else-less if return when its condition is false?

5. My Expr::If arm produces that with a RuntimeError::TypeError variant. for the case of if false {9}, only the Value::Bool(false) else_branch is evaluated, and Value::Bool(true) then_branch for the reverse scenario. In each case my arm evaluates the branch associated with the if condition being true or not. An else-less if returns Value::Unit when its condition is false.

# How I implemented it
Walk through one of your loop arms (Expr::While or Expr::For): how you destructure it, how you re-evaluate the condition or iterate the list, where you take the body's value with ? and drop it, and how you produce Value::Unit. Then describe the two new cases you added inside Expr::Unary for UnOp::Ref and UnOp::Deref. Naming the provided tools you used (self.eval_expr, self.store.alloc/read/write, env.extend, RuntimeError, Ty::reference, .into()/?) is encouraged.

1. 

Point out anything that was tricky, a bug you fixed, or a design choice you made — and if you used an assistant, say what you had it do and how you checked its work.

2. 