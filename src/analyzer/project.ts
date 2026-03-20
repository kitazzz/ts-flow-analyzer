import { Project, SourceFile } from 'ts-morph'
import { existsSync } from 'node:fs'
import { resolve } from 'node:path'

export function loadSourceFile(filePath: string): SourceFile {
  const absolutePath = resolve(filePath)
  if (!existsSync(absolutePath)) {
    throw new Error(`File not found: ${absolutePath}`)
  }

  const project = new Project({
    skipAddingFilesFromTsConfig: true,
    compilerOptions: {
      strict: true,
    },
  })

  return project.addSourceFileAtPath(absolutePath)
}
