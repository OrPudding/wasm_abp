(component
  (type $environment (instance
    (export "get-environment" (func (result (list (tuple string string)))))))
  (import "wasi:cli/environment@0.3.0" (instance (type $environment))))
