(component
  (type $os (instance
    (export "timezone-offset-minutes" (func (result s32)))
  ))
  (import "astrobox:psys-host-v4/os" (instance $os (type $os)))
  (alias export $os "timezone-offset-minutes" (func $timezone))
  (core func $lower (canon lower (func $timezone)))
  (core module $guest
    (import "" "timezone" (func $timezone (result i32)))
    (func (export "run") (drop (call $timezone)))
  )
  (core instance $guest (instantiate $guest (with "" (instance (export "timezone" (func $lower))))))
  (func (export "on-load") (canon lift (core func $guest "run")))
)
