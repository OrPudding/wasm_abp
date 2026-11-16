(component
  (type $ui (instance
    (type $size (record (field "width" u32) (field "height" u32)))
    (export "render-size" (type $public-size (eq $size)))
    (type $future (future $public-size))
    (export "get-render-size" (func (result $future)))
  ))
  (import "astrobox:psys-host/ui-v3" (instance (type $ui)))
)
