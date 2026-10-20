scalable_dir = join_paths('hicolor', 'scalable', 'apps')

install_data(
  join_paths(scalable_dir, '@0@.svg'.format(app_id)),
  install_dir: join_paths(datadir, 'icons', scalable_dir),
)

status_dir = join_paths('hicolor', 'scalable', 'status')

install_data(
  join_paths(status_dir, '@0@-symbolic.svg'.format(base_id)),
  install_dir: join_paths(datadir, 'icons', status_dir),
  rename: '@0@-symbolic.svg'.format(app_id)
)
