#!/usr/bin/env node

const path = require('path');
const glob = require('glob');

function getAllPackages() {
  return glob.sync('**/package.json', {
    ignore: ['node_modules', '.*'],
    cwd: __dirname,
  });
}

const webpackConfig = [];

for (const packageJsonPath of getAllPackages()) {
  const webpackConfigPath = path.resolve(path.dirname(packageJsonPath), 'webpack.config.js');
  webpackConfig.push(require(webpackConfigPath));
}

module.exports = webpackConfig;

