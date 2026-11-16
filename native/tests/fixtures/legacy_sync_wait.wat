(component
  (core module $memory (memory (export "memory") 1))
  (core instance $memory (instantiate $memory))
  (alias core export $memory "memory" (core memory $memory))
  (core func $new (canon waitable-set.new))
  (core func $wait (canon waitable-set.wait (memory $memory)))
  (core module $guest
    (import "" "new" (func $new (result i32)))
    (import "" "wait" (func $wait (param i32 i32) (result i32)))
    (func (export "run")
      (drop (call $wait (call $new) (i32.const 0)))))
  (core instance $guest (instantiate $guest
    (with "" (instance
      (export "new" (func $new))
      (export "wait" (func $wait))))))
  (func (export "on-load") (canon lift (core func $guest "run"))))
