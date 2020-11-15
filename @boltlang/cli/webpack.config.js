
const path = require('path');
const webpack = require('webpack');

module.exports = {
  target: 'node',
  entry: {
    'bolt': './src/main.ts'
  },
  output: {
    filename: 'bin/[name].js',
    path: path.resolve(__dirname),
    devtoolModuleFilenameTemplate: '[absolute-resource-path]'
  },
  resolve: {
    extensions: [".ts", ".js"],
  },
  plugins: [
    new webpack.BannerPlugin({ banner: '#!/usr/bin/env node', raw: true }),
  ],
  devtool: 'source-map',
  module: {
    rules: [
      {
        test: /\.ts$/,
        exclude: /node_modules/,
        loader: "ts-loader",
        options: { transpileOnly: true }
      },
      {
        test: /\.m?js$/,
        exclude: /node_modules/,
        loader: 'babel-loader'
      },
    ]
  }
};
