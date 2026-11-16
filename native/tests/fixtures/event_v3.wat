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
    (memory (export "memory") 1)
    (func (export "realloc") (param i32 i32 i32 i32) (result i32) (i32.const 4096))
    (func (export "run") (param i32 i32 i32 i32 i32) (result i32) (call $appearance))
  )
  (core instance $wrapper (instantiate $wrapper
    (with "" (instance (export "appearance" (func $lower))))
  ))
  (type $event (enum "CLICK" "CHANGE" "INPUT"))
  (type $result (future string))
  (func $callback (param "event-id" string) (param "event" $event) (param "event-payload" string) (result $result)
    (canon lift (core func $wrapper "run") (memory (core memory $wrapper "memory")) (realloc (core func $wrapper "realloc"))))
  (instance $events (export "event" (type $event)) (export "response" (type $result)) (export "on-ui-event-v3" (func $callback)))
  (export "astrobox:psys-plugin/event-v3" (instance $events))
)
