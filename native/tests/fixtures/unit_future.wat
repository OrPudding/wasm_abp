(component
  (type $timer (instance
    (type $future (future))
    (export "clear-timer" (func (param "id" u64) (result $future)))
  ))
  (import "astrobox:psys-host/timer" (instance $timer (type $timer)))
  (alias export $timer "clear-timer" (func $clear))
  (core func $lower (canon lower (func $clear)))
  (core module $wrapper
    (import "" "clear" (func $clear (param i64) (result i32)))
    (func (export "run") (result i32) (call $clear (i64.const 0)))
  )
  (core instance $wrapper (instantiate $wrapper
    (with "" (instance (export "clear" (func $lower))))
  ))
  (type $result (future))
  (func (export "on-load") (result $result) (canon lift (core func $wrapper "run")))
)
