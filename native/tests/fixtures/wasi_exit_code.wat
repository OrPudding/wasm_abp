(component
  (import "wasi:cli/exit@0.2.12" (instance $exit
    (export "exit-with-code" (func (param "status-code" u8)))
  ))
  (alias export $exit "exit-with-code" (func $exit-code))
  (core func $lower-exit (canon lower (func $exit-code)))
  (core module $guest
    (import "" "exit" (func $exit (param i32)))
    (func (export "run") (call $exit (i32.const 7)))
  )
  (core instance $guest (instantiate $guest
    (with "" (instance (export "exit" (func $lower-exit))))
  ))
  (func (export "on-load") (canon lift (core func $guest "run")))
)
