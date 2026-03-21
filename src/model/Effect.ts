export type EffectKind =
  | 'return'      // return statement
  | 'throw'       // throw statement
  | 'assignment'  // variable assignment or property mutation
  | 'call'        // function/method call (side-effectful expression statement)

export type SideEffectClass =
  | 'dbWrite'       // save/update/delete/insert/upsert/create
  | 'dbRead'        // find/findOne/get/query/fetch/load/select
  | 'externalApi'   // http/fetch/axios/request/send/publish/emit
  | 'logging'       // log/warn/error/debug/info/console
  | 'throw'         // throw — control flow termination
  | 'return'        // return — control flow termination
  | 'stateWrite'    // assignment to non-local variable or property
  | 'pureCall'      // call with no detectable side-effect class
  | 'unknown'

export type Effect = {
  kind: EffectKind
  sideEffect: SideEffectClass
  text: string    // raw source text of the statement
  line: number
}
