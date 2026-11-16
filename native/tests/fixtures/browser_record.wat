(component
  (type $browser (instance
    (type $argument (record (field "url" string)))
    (export "open-options" (type $public-argument (eq $argument)))
    (type $result (result u32 (error string)))
    (type $cookie (record (field "name" string) (field "value" string) (field "domain" string) (field "path" string)))
    (export "cookie" (type $public-cookie (eq $cookie)))
    (type $cookies (result (list $public-cookie) (error string)))
    (export "get-cookies" (func (param "id" u32) (param "url" string) (result $cookies)))
    (export "open" (func (param "options" $public-argument) (result $result)))
  ))
  (import "astrobox:psys-host-v4/browser" (instance (type $browser)))
)
