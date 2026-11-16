(component
  (type $browser (instance
    (type $argument string)
    (export "open-options" (type $public-argument (eq $argument)))
    (type $result (result u32 (error string)))
    (export "open" (func (param "options" $public-argument) (result $result)))
  ))
  (import "astrobox:psys-host-v4/browser" (instance (type $browser)))
)
