(component
  (type $account (instance
    (type $profile (record (field "username" string)))
    (export "profile" (type $public-profile (eq $profile)))
    (type $optional (option $public-profile))
    (type $result (result $optional (error string)))
    (export "get-current" (func (result $result)))
  ))
  (import "astrobox:psys-host-v4/account" (instance (type $account)))
)
