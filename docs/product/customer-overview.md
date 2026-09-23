# Casklet: Keep everything for a project together

**Casklet is in development.** This page describes the planned first release for Windows 11. It is not yet available as a finished product.

## The problem

A project rarely lives in one folder. Its documents may be in several places, while the apps and installers you used for it are elsewhere. Months later, it can be hard to remember what you added, what you still need, and what is safe to remove.

## What Casklet does

Casklet gives each activity a home called a **Cask**. You might make one for a design job, a course, a game modding project, or a temporary client assignment. In that Cask, you can bring together the files, folders, and apps related to the activity. Casklet shows you what is there, where it is, and how much space it uses.

You decide how to add a file or folder: copy it into Casklet's managed space, move it there, or simply link to it where it already lives. Linking helps you keep track of something without handing Casklet control of it. You can also group items into categories that make sense to you.

When the activity ends, Casklet prepares a cleanup preview. You can see what it proposes to remove, what it will keep, and what needs your attention before you approve anything. You can also export a list or readable report of the Cask's contents for your records.

## A simple example

Imagine you take a six-week photography course. You create a Cask for the course, add your lesson files and practice photos, and associate the editing app you already use. You can check the Cask when you want to find related material or see how much storage it takes.

After the course, you review a cleanup plan. Casklet can propose removing course files it manages, while preserving the editing app you already had and any files that are merely linked or shared with another Cask. You make the final decision after seeing the plan.

## Why it is useful

- **Find the whole picture.** See the files and software connected to an activity in one place, even when they live in different folders.
- **Understand storage use.** See how much space the activity uses and how much might actually be reclaimed. Those numbers can differ.
- **Finish projects with confidence.** Review a specific cleanup plan instead of guessing which folders and apps are safe to remove.
- **Keep a record.** Export an inventory or readable report before archiving or cleaning up an activity.
- **Stay in control.** The first release is designed to work locally on your Windows computer, without an account or cloud service.

## What makes Casklet different

Casklet organizes around an **activity**, not just a folder or a list of installed apps. It can keep track of items in different locations without requiring you to move everything into one place.

It also treats “related to this project” differently from “safe to delete.” An app you already had, a file linked from another location, or an item shared with another Cask does not become Casklet's property just because you added it to a Cask. If Casklet cannot verify that an item is safe to remove, the planned behavior is to preserve it and explain why.

## What to expect from the first release

The planned first release supports Windows 11. It will include a desktop app and command-line controls, with the same underlying behavior. Support is planned for files and folders, portable apps, Windows package management through WinGet, and cautious tracking of other installers.

Casklet is a management tool, not a protected sandbox. It cannot guarantee complete cleanup of every app installed through an arbitrary installer, and it will not automatically remove software just because you associated it with a Cask. Those limits are part of how Casklet avoids claiming certainty it does not have.
