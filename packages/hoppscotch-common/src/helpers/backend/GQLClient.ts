import { ref } from "vue"
import {
  TypedDocumentNode,
  OperationContext,
  createRequest,
  CombinedError,
  Operation,
  OperationResult,
  Client,
  AnyVariables,
} from "@urql/core"
import * as E from "fp-ts/Either"
import * as TE from "fp-ts/TaskEither"
import { pipe, constVoid, flow } from "fp-ts/function"
import { subscribe, pipe as wonkaPipe } from "wonka"
import { filter, map, Subject } from "rxjs"

type GQLOpType = "query" | "mutation" | "subscription"
/**
 * A type that defines error events that are possible during backend operations on the GQLCLient
 */
export type GQLClientErrorEvent =
  | { type: "SUBSCRIPTION_CONN_CALLBACK_ERR_REPORT"; errors: Error[] }
  | { type: "CLIENT_REPORTED_ERROR"; error: CombinedError; op: Operation }
  | {
      type: "GQL_CLIENT_REPORTED_ERROR"
      opType: GQLOpType
      opResult: OperationResult
    }

/**
 * A stream of the errors that occur during GQLClient operations.
 * Exposed to be subscribed to by systems like sentry for error reporting
 */
export const gqlClientError$ = new Subject<GQLClientErrorEvent>()

export const client = ref<Client>()

type RunQueryOptions<T = any, V = AnyVariables> = {
  query: TypedDocumentNode<T, V>
  variables: V
}

/**
 * A wrapper type for defining errors possible in a GQL operation
 */
export type GQLError<T extends string> =
  | {
      type: "network_error"
      error: Error
    }
  | {
      type: "gql_error"
      error: T
    }

export const runGQLQuery = <
  DocType,
  DocVarType extends AnyVariables,
  DocErrorType extends string,
>(
  args: RunQueryOptions<DocType, DocVarType>
): Promise<E.Either<GQLError<DocErrorType>, DocType>> => {
  const request = createRequest<DocType, DocVarType>(args.query, args.variables)
  const source = client.value!.executeQuery(request, {
    requestPolicy: "network-only",
  })

  return new Promise((resolve) => {
    const sub = wonkaPipe(
      source,
      subscribe((res) => {
        if (sub) {
          sub.unsubscribe()
        }

        pipe(
          // The target
          res.data as DocType | undefined,
          // Define what happens if data does not exist (it is an error)
          E.fromNullable(
            pipe(
              // Take the network error value
              res.error?.networkError,
              // If it null, set the left to the generic error name
              E.fromNullable(res.error?.message),
              E.match(
                // The left case (network error was null)
                (gqlErr) => {
                  if (res.error) {
                    gqlClientError$.next({
                      type: "GQL_CLIENT_REPORTED_ERROR",
                      opType: "query",
                      opResult: res,
                    })
                  }

                  return <GQLError<DocErrorType>>{
                    type: "gql_error",
                    error: parseGQLErrorString(gqlErr ?? "") as DocErrorType,
                  }
                },
                // The right case (it was a GraphQL Error)
                (networkErr) =>
                  <GQLError<DocErrorType>>{
                    type: "network_error",
                    error: networkErr,
                  }
              )
            )
          ),
          resolve
        )
      })
    )
  })
}

// TODO: The subscription system seems to be firing multiple updates for certain subscriptions.
// Make sure to handle cases if the subscription fires with the same update multiple times
export const runGQLSubscription = <
  DocType,
  DocVarType extends AnyVariables,
  DocErrorType extends string,
>(
  args: RunQueryOptions<DocType, DocVarType>
) => {
  const result$ = new Subject<E.Either<GQLError<DocErrorType>, DocType>>()

  const source = client.value!.executeSubscription(
    createRequest(args.query, args.variables)
  )

  const sub = wonkaPipe(
    source,
    subscribe((res) => {
      result$.next(
        pipe(
          // The target
          res.data as DocType | undefined,
          // Define what happens if data does not exist (it is an error)
          E.fromNullable(
            pipe(
              // Take the network error value
              res.error?.networkError,
              // If it null, set the left to the generic error name
              E.fromNullable(res.error?.message),
              E.match(
                // The left case (network error was null)
                (gqlErr) => {
                  if (res.error) {
                    gqlClientError$.next({
                      type: "GQL_CLIENT_REPORTED_ERROR",
                      opType: "subscription",
                      opResult: res,
                    })
                  }

                  return <GQLError<DocErrorType>>{
                    type: "gql_error",
                    error: parseGQLErrorString(gqlErr ?? "") as DocErrorType,
                  }
                },
                // The right case (it was a GraphQL Error)
                (networkErr) =>
                  <GQLError<DocErrorType>>{
                    type: "network_error",
                    error: networkErr,
                  }
              )
            )
          )
        )
      )
    })
  )

  // Returns the stream and a subscription handle to unsub
  return [result$, sub] as const
}

/**
 * Same as `runGQLSubscription` but stops the subscription silently
 * if there is an authentication error because of logged out
 */
export const runAuthOnlyGQLSubscription = flow(
  runGQLSubscription,
  ([result$, sub]) => {
    const updatedResult$ = result$.pipe(
      map((res) => {
        if (
          E.isLeft(res) &&
          res.left.type === "gql_error" &&
          res.left.error === "auth/fail"
        ) {
          sub.unsubscribe()
          return null
        }
        return res
      }),
      filter((res): res is Exclude<typeof res, null> => res !== null)
    )

    return [updatedResult$, sub] as const
  }
)

export const parseGQLErrorString = (s: string) =>
  s.startsWith("[GraphQL] ") ? s.split("[GraphQL] ")[1] : s

export const runMutation = <
  DocType,
  DocVariables extends object | undefined,
  DocErrors extends string,
>(
  mutation: TypedDocumentNode<DocType, DocVariables>,
  variables: DocVariables,
  additionalConfig?: Partial<OperationContext>
): TE.TaskEither<GQLError<DocErrors>, DocType> =>
  pipe(
    TE.tryCatch(
      () =>
        client
          .value!.mutation(mutation, variables, {
            requestPolicy: "cache-and-network",
            ...additionalConfig,
          })
          .toPromise(),
      () => constVoid() as never // The mutation function can never fail, so this will never be called ;)
    ),
    TE.chainEitherK((result) =>
      pipe(
        result.data,
        E.fromNullable(
          // Result is null
          pipe(
            result.error?.networkError,
            E.fromNullable(result.error?.message),
            E.match(
              // The left case (network error was null)
              (gqlErr) => {
                if (result.error) {
                  gqlClientError$.next({
                    type: "GQL_CLIENT_REPORTED_ERROR",
                    opType: "mutation",
                    opResult: result,
                  })
                }

                return <GQLError<DocErrors>>{
                  type: "gql_error",
                  error: parseGQLErrorString(gqlErr ?? ""),
                }
              },
              // The right case (it was a network error)
              (networkErr) =>
                <GQLError<DocErrors>>{
                  type: "network_error",
                  error: networkErr,
                }
            )
          )
        )
      )
    )
  )