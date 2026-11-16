(component
  (type $os (instance
    (type $future (future string))
    (export "appearance" (func (result $future)))
  ))
  (import "astrobox:psys-host/os" (instance $os (type $os)))
  (alias export $os "appearance" (func $appearance))
  (core func $lower (canon lower (func $appearance)))
  (core module $wrapper
    (import "" "appearance" (func $appearance (result i32)))
    (func (export "run") (result i32) (call $appearance))
  )
  (core instance $wrapper (instantiate $wrapper
    (with "" (instance (export "appearance" (func $lower))))
  ))
  (type $result (future string))
  (func (export "on-load") (result $result)
    (canon lift (core func $wrapper "run")))
)
