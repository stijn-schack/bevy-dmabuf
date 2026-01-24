# Bevy external buffer import plugin

<!-- TOC -->
* [Bevy external buffer import plugin](#bevy-external-buffer-import-plugin)
  * [About](#about)
  * [Supported Platforms](#supported-platforms)
  * [Features](#features)
  * [Missing features & known issues](#missing-features--known-issues)
<!-- TOC -->

## About

This Bevy plugin makes it possible to import buffers from different processes and/or graphics contexts.
Originally a fork of [Schmarni's bevy-dmabuf](https://github.com/Schmarni-Dev/bevy-dmabuf/), but heavily modified to
*hopefully* increase stability and UX.

Importing externally allocated buffers is a complex topic,
and WGPU (the gfx library used by Bevy's renderer under the hood), has minimal support for it.
This also means that **this plugin contains unsafe code to interact with wgpu-hal**.

While I did learn a lot by working on this plugin, I am by no means an expert on low-level gfx programming, so keep that
in mind if you want to use this plugin.

## Supported Platforms

Currently only Linux is supported with the Vulkan wgpu backend.
Personally I have no motivation to provide OpenGL integration and I do not have access to other operating systems at the
moment,
so feel free fork this repository & contribute platform-specific code if you have the know-how.

## Features

Out of the box, this plugin provides two ways to import a buffer depending on how you intend to use it.

- Sample as a texture, exposed as a `Handle<Image>`
- As a camera render target, using bevy's built-in `RenderTarget::TextureView`

Both use cases have a dedicated demo in [examples](examples).

## Missing features & known issues

There is no support for explicit synchronization/fences yet.
I am still working out a way to implement this cleanly.
